#!/usr/bin/env python3
"""Fail-closed dependency and private-source boundary checks for NQ.

The resolved Cargo graph is authoritative.  The checker deliberately includes
normal, development, build, and target-qualified dependency edges from
``cargo metadata --locked --format-version 1``.

Adding a constellation component should require a table entry below, not new
graph-walking code:

* ``PACKAGE_ROLES`` / ``PACKAGE_ROLE_PREFIXES`` classify packages.
* ``FORBIDDEN_REACHABLE_ROLES`` and ``FORBIDDEN_EXACT_REACHABILITY`` state the
  one-directional dependency law.
* ``ROLE_EXTERNAL_DEPENDENCIES`` keeps deliberately small leaves small.
* ``REQUIRED_DIRECT_CONTROLS`` proves that the resolved graph was populated.

The source scan is a second line of defence.  Cargo edges are legitimate public
package boundaries; relative includes, target paths, or string-literal source
paths that enter a sibling package are not.  Existing violations are recorded
in ``TRANSITIONAL_COUPLING_ALLOWLIST`` with both a reason and a removal
condition.  The allowlist is exact and self-expiring: an unknown violation and
an allowance which no longer matches a violation both fail the gate.

No third-party Python modules are used.  ``--self-test`` exercises the checker
without invoking Cargo.
"""

from __future__ import annotations

import argparse
import collections
import dataclasses
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from typing import Iterable, Mapping, Sequence


# Semantic package roles.  Exact names win over prefixes.
PACKAGE_ROLES: Mapping[str, str] = {
    # Stable target components.
    "nq-protocol": "protocol",
    "nq": "decision",
    "nq-decision": "decision",
    "nq-witness": "witness_artifact",
    "nq-witness-artifact": "witness_artifact",
    "nq-witness-api": "witness_transport",
    "nq-monitor-agent": "monitor_agent",
    "nq-monitor": "monitor_runtime",
    "nq-dashboard": "dashboard",
    "nq-suite": "composition",
    "nq-runtime": "composition",
    "nq-cli": "composition",
    # Transitional packages.  These names make the current ownership debt
    # visible without pretending that either package is a clean target role.
    "nq-core": "transitional_core",
    "nq-db": "storage",
}

PACKAGE_ROLE_PREFIXES: Sequence[tuple[str, str]] = (
    ("nq-check-pack-", "check_pack"),
    ("nq-check-", "check_pack"),
    ("nq-pack-", "check_pack"),
)


# A source role may not reach any target role listed here, even through dev,
# build, or target-qualified intermediate edges.
FORBIDDEN_REACHABLE_ROLES: Mapping[str, frozenset[str]] = {
    "protocol": frozenset(
        {
            "decision",
            "witness_artifact",
            "witness_transport",
            "monitor_agent",
            "monitor_runtime",
            "dashboard",
            "storage",
            "check_pack",
            "composition",
            "transitional_core",
        }
    ),
    "decision": frozenset(
        {
            "storage",
            "monitor_agent",
            "monitor_runtime",
            "dashboard",
            "check_pack",
            "composition",
        }
    ),
    "witness_artifact": frozenset(
        {
            "decision",
            "storage",
            "witness_transport",
            "monitor_agent",
            "monitor_runtime",
            "dashboard",
            "check_pack",
            "composition",
            "transitional_core",
        }
    ),
    "witness_transport": frozenset({"decision", "storage", "composition"}),
    "monitor_agent": frozenset(
        {"decision", "storage", "dashboard", "composition"}
    ),
    "dashboard": frozenset({"storage", "composition"}),
    "check_pack": frozenset({"decision", "storage", "dashboard", "composition"}),
    "storage": frozenset(
        {"monitor_agent", "monitor_runtime", "dashboard", "check_pack", "composition"}
    ),
}

# Exact assertions remain useful while mixed transitional roles exist.
FORBIDDEN_EXACT_REACHABILITY: Mapping[tuple[str, str], str] = {
    (
        "nq-monitor-agent",
        "nq-db",
    ): "collection/execution must not reach the finding database",
    (
        "nq-witness-api",
        "nq-db",
    ): "witness transport must not reach the finding database",
}


@dataclasses.dataclass(frozen=True)
class DependencyAllowance:
    """One exact, self-expiring transitional dependency path."""

    path: tuple[str, ...]
    reason: str
    removal_condition: str


TRANSITIONAL_DEPENDENCY_ALLOWLIST: Sequence[DependencyAllowance] = (
    DependencyAllowance(
        ("nq-monitor-agent", "nq-core", "nq"),
        (
            "nq-core temporarily reexports the frozen decision receipt types "
            "while the monitor agent still imports mixed monitor/config DTOs "
            "from nq-core"
        ),
        (
            "remove when nq-monitor-agent consumes monitor-owned DTOs and no "
            "longer depends on the transitional nq-core package"
        ),
    ),
    DependencyAllowance(
        ("nq-witness-api", "nq-core", "nq"),
        (
            "nq-core temporarily reexports the frozen refusal and disposition "
            "types while witness transport still imports mixed preflight DTOs "
            "from nq-core"
        ),
        (
            "remove when nq-witness-api consumes witness/monitor boundary DTOs "
            "without depending on the transitional nq-core package"
        ),
    ),
)


@dataclasses.dataclass(frozen=True)
class DirectControl:
    consumer: str
    dependency: str
    kind: str
    reason: str


REQUIRED_DIRECT_CONTROLS: Sequence[DirectControl] = (
    DirectControl(
        consumer="nq-monitor",
        dependency="nq-db",
        kind="normal",
        reason=(
            "transitional positive control: the current runtime directly uses "
            "the database, proving the resolved graph is not empty"
        ),
    ),
)

# Only direct external dependencies are constrained here.  Transitive
# dependencies of serde/thiserror/time remain the responsibility of those
# packages and Cargo.lock.
ROLE_EXTERNAL_DEPENDENCIES: Mapping[str, frozenset[str]] = {
    "protocol": frozenset({"serde", "thiserror", "time"}),
}


@dataclasses.dataclass(frozen=True)
class CouplingAllowance:
    kind: str
    consumer: str
    source_path: str
    target: str
    target_path: str
    reason: str
    removal_condition: str

    @property
    def key(self) -> tuple[str, str, str, str, str]:
        return (
            self.kind,
            self.consumer,
            self.source_path,
            self.target,
            self.target_path,
        )


_INQUIRY_FIXTURE_REASON = (
    "incremental extraction still compiles nq-core inquiry test vectors into "
    "a consumer package"
)
_INQUIRY_FIXTURE_REMOVAL = (
    "remove when versioned inquiry fixtures are owned by the decision artifact "
    "boundary and consumers load them through that package's public test contract"
)

TRANSITIONAL_COUPLING_ALLOWLIST: Sequence[CouplingAllowance] = (
    CouplingAllowance(
        "cross-package-include",
        "nq-db",
        "crates/nq-db/src/inquiry.rs",
        "nq-core",
        (
            "crates/nq-core/tests/fixtures/"
            "resolver_pending_aged_tail.profile_catalog.v0.json"
        ),
        _INQUIRY_FIXTURE_REASON,
        _INQUIRY_FIXTURE_REMOVAL,
    ),
    CouplingAllowance(
        "cross-package-include",
        "nq-monitor",
        "crates/nq-monitor/src/cmd/emit_escalation.rs",
        "nq-core",
        (
            "crates/nq-core/tests/fixtures/"
            "resolver_pending_aged_tail.profile_catalog.v0.json"
        ),
        _INQUIRY_FIXTURE_REASON,
        _INQUIRY_FIXTURE_REMOVAL,
    ),
    CouplingAllowance(
        "cross-package-include",
        "nq-monitor",
        "crates/nq-monitor/src/cmd/inquire.rs",
        "nq-core",
        (
            "crates/nq-core/tests/fixtures/"
            "resolver_pending_aged_tail.profile_catalog.v0.json"
        ),
        _INQUIRY_FIXTURE_REASON,
        _INQUIRY_FIXTURE_REMOVAL,
    ),
    CouplingAllowance(
        "cross-package-include",
        "nq-monitor",
        "crates/nq-monitor/src/cmd/inquire.rs",
        "nq-core",
        "crates/nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json",
        _INQUIRY_FIXTURE_REASON,
        _INQUIRY_FIXTURE_REMOVAL,
    ),
    CouplingAllowance(
        "cross-package-include",
        "nq-monitor",
        "crates/nq-monitor/src/cmd/intent.rs",
        "nq-core",
        "crates/nq-core/tests/fixtures/golden_success.inquiry_intent.v0.json",
        _INQUIRY_FIXTURE_REASON,
        _INQUIRY_FIXTURE_REMOVAL,
    ),
    CouplingAllowance(
        "cross-package-include",
        "nq-monitor",
        "crates/nq-monitor/src/cmd/intent.rs",
        "nq-core",
        (
            "crates/nq-core/tests/fixtures/"
            "tls_cert_ambiguous.profile_catalog.v0.json"
        ),
        _INQUIRY_FIXTURE_REASON,
        _INQUIRY_FIXTURE_REMOVAL,
    ),
    CouplingAllowance(
        "cross-package-include",
        "nq-monitor",
        "crates/nq-monitor/src/cmd/intent.rs",
        "nq-core",
        "crates/nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json",
        _INQUIRY_FIXTURE_REASON,
        _INQUIRY_FIXTURE_REMOVAL,
    ),
    CouplingAllowance(
        "repository-external-include",
        "nq-core",
        "crates/nq-core/tests/reliance_conformance.rs",
        "@repository",
        "docs/examples/reliance-profiles.json",
        (
            "nq-core's integration test still compiles the repository-level "
            "reliance example"
        ),
        (
            "remove when the versioned reliance vector is packaged beneath its "
            "authoritative decision component"
        ),
    ),
)


@dataclasses.dataclass(frozen=True)
class PackageRecord:
    package_id: str
    name: str
    manifest_path: Path
    source: str | None

    @property
    def root(self) -> Path:
        return self.manifest_path.parent


@dataclasses.dataclass
class EdgeInfo:
    kinds: set[str] = dataclasses.field(default_factory=set)
    targets: set[str] = dataclasses.field(default_factory=set)

    def render(self) -> str:
        kinds = ",".join(sorted(self.kinds))
        targets = ",".join(sorted(self.targets))
        return f"kinds={kinds}; targets={targets}"


@dataclasses.dataclass
class GraphModel:
    packages: dict[str, PackageRecord]
    local_ids: set[str]
    edges: dict[tuple[str, str], EdgeInfo]
    ids_by_name: dict[str, list[str]]

    def local_successors(self, package_id: str) -> list[str]:
        return sorted(
            dst
            for src, dst in self.edges
            if src == package_id and dst in self.local_ids
        )


@dataclasses.dataclass(frozen=True)
class Violation:
    code: str
    message: str


@dataclasses.dataclass(frozen=True)
class CouplingFinding:
    kind: str
    consumer: str
    source_path: str
    target: str
    target_path: str
    line: int
    expression: str

    @property
    def key(self) -> tuple[str, str, str, str, str]:
        return (
            self.kind,
            self.consumer,
            self.source_path,
            self.target,
            self.target_path,
        )


@dataclasses.dataclass(frozen=True)
class SourceAudit:
    findings: tuple[CouplingFinding, ...]
    allowed: tuple[tuple[CouplingAllowance, int], ...]
    violations: tuple[Violation, ...]


def role_for(package_name: str) -> str | None:
    exact = PACKAGE_ROLES.get(package_name)
    if exact is not None:
        return exact
    for prefix, role in PACKAGE_ROLE_PREFIXES:
        if package_name.startswith(prefix):
            return role
    return None


def _normalise_kind(kind: object) -> str:
    return "normal" if kind is None else str(kind)


def graph_from_metadata(metadata: Mapping[str, object]) -> GraphModel:
    raw_packages = metadata.get("packages")
    raw_resolve = metadata.get("resolve")
    if not isinstance(raw_packages, list):
        raise ValueError("metadata.packages must be a list")
    if not isinstance(raw_resolve, dict):
        raise ValueError("metadata.resolve must be an object (not null)")

    packages: dict[str, PackageRecord] = {}
    ids_by_name: dict[str, list[str]] = collections.defaultdict(list)
    for raw in raw_packages:
        if not isinstance(raw, dict):
            raise ValueError("each metadata package must be an object")
        package_id = str(raw.get("id", ""))
        name = str(raw.get("name", ""))
        manifest = str(raw.get("manifest_path", ""))
        if not package_id or not name or not manifest:
            raise ValueError("metadata package lacks id, name, or manifest_path")
        source_value = raw.get("source")
        source = None if source_value is None else str(source_value)
        record = PackageRecord(package_id, name, Path(manifest).resolve(), source)
        packages[package_id] = record
        ids_by_name[name].append(package_id)

    edges: dict[tuple[str, str], EdgeInfo] = {}
    nodes = raw_resolve.get("nodes")
    if not isinstance(nodes, list):
        raise ValueError("metadata.resolve.nodes must be a list")
    for node in nodes:
        if not isinstance(node, dict):
            raise ValueError("each resolve node must be an object")
        source_id = str(node.get("id", ""))
        if source_id not in packages:
            raise ValueError(f"resolve node names unknown package id {source_id!r}")
        raw_deps = node.get("deps", [])
        if not isinstance(raw_deps, list):
            raise ValueError(f"resolve node {source_id!r} deps must be a list")
        for raw_dep in raw_deps:
            if not isinstance(raw_dep, dict):
                raise ValueError(f"resolve node {source_id!r} has non-object dep")
            target_id = str(raw_dep.get("pkg", ""))
            if target_id not in packages:
                raise ValueError(
                    f"resolve edge {source_id!r} names unknown package {target_id!r}"
                )
            dep_kinds = raw_dep.get("dep_kinds")
            if not isinstance(dep_kinds, list) or not dep_kinds:
                dep_kinds = [{"kind": None, "target": None}]
            info = edges.setdefault((source_id, target_id), EdgeInfo())
            for dep_kind in dep_kinds:
                if not isinstance(dep_kind, dict):
                    raise ValueError(
                        f"resolve edge {source_id!r}->{target_id!r} has bad dep_kind"
                    )
                info.kinds.add(_normalise_kind(dep_kind.get("kind")))
                target = dep_kind.get("target")
                info.targets.add("<all-targets>" if target is None else str(target))

    local_ids = {
        package_id
        for package_id, package in packages.items()
        if package.source is None
    }
    return GraphModel(packages, local_ids, edges, dict(ids_by_name))


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _local_path_violations(graph: GraphModel, repository: Path) -> list[Violation]:
    violations: list[Violation] = []
    for package_id in sorted(graph.local_ids):
        package = graph.packages[package_id]
        if not _is_within(package.manifest_path, repository):
            violations.append(
                Violation(
                    "LOCAL_PACKAGE_OUTSIDE_REPOSITORY",
                    (
                        f"{package.name} resolves from {package.manifest_path}, outside "
                        f"{repository}; installation would rely on a sibling checkout"
                    ),
                )
            )
    return violations


def _role_violations(graph: GraphModel) -> list[Violation]:
    violations: list[Violation] = []
    for package_id in sorted(graph.local_ids):
        package = graph.packages[package_id]
        if role_for(package.name) is None:
            violations.append(
                Violation(
                    "UNKNOWN_PACKAGE_ROLE",
                    (
                        f"local package {package.name!r} has no semantic role; add an "
                        "exact or prefix role before it can enter the constellation"
                    ),
                )
            )
    return violations


def _strong_components(graph: GraphModel) -> list[list[str]]:
    index = 0
    indices: dict[str, int] = {}
    lowlinks: dict[str, int] = {}
    stack: list[str] = []
    on_stack: set[str] = set()
    result: list[list[str]] = []

    def visit(node: str) -> None:
        nonlocal index
        indices[node] = index
        lowlinks[node] = index
        index += 1
        stack.append(node)
        on_stack.add(node)

        for successor in graph.local_successors(node):
            if successor not in indices:
                visit(successor)
                lowlinks[node] = min(lowlinks[node], lowlinks[successor])
            elif successor in on_stack:
                lowlinks[node] = min(lowlinks[node], indices[successor])

        if lowlinks[node] == indices[node]:
            component: list[str] = []
            while True:
                member = stack.pop()
                on_stack.remove(member)
                component.append(member)
                if member == node:
                    break
            result.append(component)

    for package_id in sorted(graph.local_ids):
        if package_id not in indices:
            visit(package_id)
    return result


def _cycle_violations(graph: GraphModel) -> list[Violation]:
    violations: list[Violation] = []
    for component in _strong_components(graph):
        self_loop = (
            len(component) == 1 and (component[0], component[0]) in graph.edges
        )
        if len(component) <= 1 and not self_loop:
            continue
        members = set(component)
        edge_descriptions: list[str] = []
        for (source, target), info in sorted(graph.edges.items()):
            if source in members and target in members:
                edge_descriptions.append(
                    (
                        f"{graph.packages[source].name}->{graph.packages[target].name} "
                        f"({info.render()})"
                    )
                )
        names = ", ".join(sorted(graph.packages[item].name for item in component))
        violations.append(
            Violation(
                "DEPENDENCY_CYCLE",
                f"local dependency cycle among [{names}]: "
                + "; ".join(edge_descriptions),
            )
        )
    return violations


def _path_between(
    graph: GraphModel, source: str, target: str
) -> list[str] | None:
    queue: collections.deque[str] = collections.deque([source])
    previous: dict[str, str | None] = {source: None}
    while queue:
        current = queue.popleft()
        if current == target:
            path: list[str] = []
            cursor: str | None = current
            while cursor is not None:
                path.append(cursor)
                cursor = previous[cursor]
            return list(reversed(path))
        for successor in graph.local_successors(current):
            if successor not in previous:
                previous[successor] = current
                queue.append(successor)
    return None


def _render_path(graph: GraphModel, path: Sequence[str]) -> str:
    rendered: list[str] = []
    for index, package_id in enumerate(path):
        rendered.append(graph.packages[package_id].name)
        if index + 1 < len(path):
            edge = graph.edges[(package_id, path[index + 1])]
            rendered.append(f"-[{edge.render()}]->")
    return " ".join(rendered)


def _forbidden_dependency_violations(graph: GraphModel) -> list[Violation]:
    violations: list[Violation] = []
    emitted: set[tuple[str, str]] = set()
    applicable_allowances: dict[tuple[str, ...], DependencyAllowance] = {}
    for allowance in TRANSITIONAL_DEPENDENCY_ALLOWLIST:
        if len(allowance.path) < 2:
            violations.append(
                Violation(
                    "MALFORMED_DEPENDENCY_ALLOWANCE",
                    f"dependency allowance path {allowance.path!r} is too short",
                )
            )
            continue
        if not allowance.reason.strip() or not allowance.removal_condition.strip():
            violations.append(
                Violation(
                    "MALFORMED_DEPENDENCY_ALLOWANCE",
                    (
                        f"dependency allowance {allowance.path!r} requires a "
                        "reason and removal condition"
                    ),
                )
            )
            continue
        if allowance.path in applicable_allowances:
            violations.append(
                Violation(
                    "DUPLICATE_DEPENDENCY_ALLOWANCE",
                    f"duplicate dependency allowance {allowance.path!r}",
                )
            )
            continue
        if all(_single_local_id(graph, name) is not None for name in allowance.path):
            applicable_allowances[allowance.path] = allowance

    matched_allowances: set[tuple[str, ...]] = set()
    for source_id in sorted(graph.local_ids):
        source = graph.packages[source_id]
        source_role = role_for(source.name)
        if source_role is None:
            continue
        for target_id in sorted(graph.local_ids):
            if target_id == source_id:
                continue
            target = graph.packages[target_id]
            target_role = role_for(target.name)
            exact_reason = FORBIDDEN_EXACT_REACHABILITY.get(
                (source.name, target.name)
            )
            role_forbidden = (
                target_role is not None
                and target_role
                in FORBIDDEN_REACHABLE_ROLES.get(source_role, frozenset())
            )
            if exact_reason is None and not role_forbidden:
                continue
            path = _path_between(graph, source_id, target_id)
            if path is None or (source_id, target_id) in emitted:
                continue
            emitted.add((source_id, target_id))
            path_names = tuple(graph.packages[item].name for item in path)
            if path_names in applicable_allowances:
                matched_allowances.add(path_names)
                continue
            reason = (
                exact_reason
                if exact_reason is not None
                else (
                    f"role {source_role!r} may not reach role "
                    f"{target_role!r}"
                )
            )
            violations.append(
                Violation(
                    "FORBIDDEN_DEPENDENCY",
                    f"{reason}: {_render_path(graph, path)}",
                )
            )
    for path, allowance in sorted(applicable_allowances.items()):
        if path not in matched_allowances:
            violations.append(
                Violation(
                    "STALE_OR_CHANGED_DEPENDENCY_ALLOWANCE",
                    (
                        f"dependency allowance {path!r} no longer matches the "
                        "exact forbidden path; remove it or update the migration. "
                        f"Removal condition: {allowance.removal_condition}"
                    ),
                )
            )
    return violations


def _active_dependency_allowances(
    graph: GraphModel,
) -> list[DependencyAllowance]:
    active: list[DependencyAllowance] = []
    for allowance in TRANSITIONAL_DEPENDENCY_ALLOWLIST:
        ids = [_single_local_id(graph, name) for name in allowance.path]
        if any(item is None for item in ids):
            continue
        resolved = _path_between(graph, ids[0], ids[-1])  # type: ignore[arg-type]
        if resolved is None:
            continue
        names = tuple(graph.packages[item].name for item in resolved)
        if names == allowance.path:
            active.append(allowance)
    return active


def _external_dependency_violations(graph: GraphModel) -> list[Violation]:
    violations: list[Violation] = []
    for source_id in sorted(graph.local_ids):
        source = graph.packages[source_id]
        role = role_for(source.name)
        allowed = ROLE_EXTERNAL_DEPENDENCIES.get(role or "")
        if allowed is None:
            continue
        for (edge_source, target_id), info in sorted(graph.edges.items()):
            if edge_source != source_id or target_id in graph.local_ids:
                continue
            target = graph.packages[target_id]
            if target.name not in allowed:
                violations.append(
                    Violation(
                        "FORBIDDEN_EXTERNAL_DEPENDENCY",
                        (
                            f"{source.name} role {role!r} directly depends on "
                            f"external {target.name!r} ({info.render()}); allowed "
                            f"external packages are {sorted(allowed)}"
                        ),
                    )
                )
    return violations


def _single_local_id(graph: GraphModel, name: str) -> str | None:
    matches = [
        item for item in graph.ids_by_name.get(name, []) if item in graph.local_ids
    ]
    if len(matches) != 1:
        return None
    return matches[0]


def _control_violations(graph: GraphModel) -> list[Violation]:
    violations: list[Violation] = []
    for control in REQUIRED_DIRECT_CONTROLS:
        source_id = _single_local_id(graph, control.consumer)
        target_id = _single_local_id(graph, control.dependency)
        if source_id is None or target_id is None:
            violations.append(
                Violation(
                    "MISSING_POSITIVE_CONTROL_PACKAGE",
                    (
                        f"control {control.consumer}->{control.dependency} cannot "
                        "resolve exactly one local package at each endpoint"
                    ),
                )
            )
            continue
        info = graph.edges.get((source_id, target_id))
        if info is None or control.kind not in info.kinds:
            found = "no edge" if info is None else info.render()
            violations.append(
                Violation(
                    "MISSING_POSITIVE_CONTROL_EDGE",
                    (
                        f"expected direct {control.kind} edge "
                        f"{control.consumer}->{control.dependency}: {control.reason}; "
                        f"found {found}"
                    ),
                )
            )
    return violations


def analyze_metadata(
    metadata: Mapping[str, object],
    repository: Path,
    *,
    enforce_controls: bool = True,
) -> tuple[GraphModel, list[Violation]]:
    repository = repository.resolve()
    graph = graph_from_metadata(metadata)
    violations: list[Violation] = []
    violations.extend(_local_path_violations(graph, repository))
    violations.extend(_role_violations(graph))
    violations.extend(_cycle_violations(graph))
    violations.extend(_forbidden_dependency_violations(graph))
    violations.extend(_external_dependency_violations(graph))
    if enforce_controls:
        violations.extend(_control_violations(graph))
    return graph, violations


_PACKAGE_SECTION_RE = re.compile(
    r"(?ms)^\[package\]\s*(.*?)(?=^\[[^\n]+\]\s*$|\Z)"
)
_PACKAGE_NAME_RE = re.compile(r'(?m)^\s*name\s*=\s*"([^"]+)"\s*$')
_MANIFEST_PATH_RE = re.compile(r'\bpath\s*=\s*"([^"]+)"')
_INCLUDE_RE = re.compile(
    r"\b(include|include_str|include_bytes)!\s*\(\s*\"([^\"\n]+)\"",
    re.MULTILINE,
)
_PATH_ATTRIBUTE_RE = re.compile(
    r"#\s*\[\s*path\s*=\s*\"([^\"\n]+)\"\s*\]",
    re.MULTILINE,
)
_SIBLING_SRC_LITERAL_RE = re.compile(
    r"\"((?:\.\./)+[^\"\n]*?/src(?:/[^\"\n]*)?)\""
)
_SOURCE_SUFFIXES = frozenset({".rs", ".py", ".sh"})


def _read_package_name(manifest: Path) -> str | None:
    try:
        text = manifest.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return None
    section = _PACKAGE_SECTION_RE.search(text)
    if section is None:
        return None
    name = _PACKAGE_NAME_RE.search(section.group(1))
    return None if name is None else name.group(1)


def _walk_without_build_artifacts(root: Path) -> Iterable[Path]:
    for directory, subdirs, files in os.walk(root):
        subdirs[:] = sorted(
            item
            for item in subdirs
            if item not in {".git", "target", "__pycache__"}
        )
        for filename in sorted(files):
            yield Path(directory) / filename


def discover_package_roots(repository: Path) -> dict[str, Path]:
    repository = repository.resolve()
    roots: dict[str, Path] = {}
    for candidate in _walk_without_build_artifacts(repository):
        if candidate.name != "Cargo.toml":
            continue
        name = _read_package_name(candidate)
        if name is None:
            continue
        if name in roots:
            raise ValueError(
                f"duplicate package name {name!r}: {roots[name]} and {candidate.parent}"
            )
        roots[name] = candidate.parent.resolve()
    return roots


def _owner_for(path: Path, package_roots: Mapping[str, Path]) -> str | None:
    candidates = [
        (len(str(root)), name)
        for name, root in package_roots.items()
        if _is_within(path, root)
    ]
    if not candidates:
        return None
    return max(candidates)[1]


def _repository_path(path: Path, repository: Path) -> str:
    try:
        return path.relative_to(repository).as_posix()
    except ValueError:
        return str(path)


def _finding_for_external_path(
    *,
    repository: Path,
    package_roots: Mapping[str, Path],
    consumer: str,
    source: Path,
    target_path: Path,
    line: int,
    expression: str,
    cross_package_kind: str,
    repository_kind: str,
    external_kind: str,
) -> CouplingFinding | None:
    consumer_root = package_roots[consumer]
    if _is_within(target_path, consumer_root):
        return None
    target_owner = _owner_for(target_path, package_roots)
    source_rel = _repository_path(source, repository)
    if target_owner is not None:
        kind = cross_package_kind
        target = target_owner
    elif _is_within(target_path, repository):
        kind = repository_kind
        target = "@repository"
    else:
        kind = external_kind
        target = "@external"
    return CouplingFinding(
        kind=kind,
        consumer=consumer,
        source_path=source_rel,
        target=target,
        target_path=_repository_path(target_path, repository),
        line=line,
        expression=expression,
    )


def scan_source_couplings(
    repository: Path, package_roots: Mapping[str, Path]
) -> list[CouplingFinding]:
    repository = repository.resolve()
    findings: list[CouplingFinding] = []
    seen_files: set[Path] = set()

    for consumer, root in sorted(package_roots.items()):
        manifest = root / "Cargo.toml"
        if manifest.exists():
            text = manifest.read_text(encoding="utf-8")
            for match in _MANIFEST_PATH_RE.finditer(text):
                literal = match.group(1)
                target_path = (manifest.parent / literal).resolve()
                target_owner = _owner_for(target_path, package_roots)
                if target_owner == consumer:
                    continue
                # A path dependency naming another package root is a public
                # package edge and is checked in cargo metadata.
                if target_owner is not None and target_path == package_roots[target_owner]:
                    continue
                finding = _finding_for_external_path(
                    repository=repository,
                    package_roots=package_roots,
                    consumer=consumer,
                    source=manifest,
                    target_path=target_path,
                    line=text.count("\n", 0, match.start()) + 1,
                    expression=f'path = "{literal}"',
                    cross_package_kind="manifest-private-path",
                    repository_kind="manifest-repository-private-path",
                    external_kind="manifest-external-path",
                )
                if finding is not None:
                    findings.append(finding)

        for source in _walk_without_build_artifacts(root):
            if source in seen_files or source.suffix not in _SOURCE_SUFFIXES:
                continue
            seen_files.add(source)
            try:
                text = source.read_text(encoding="utf-8")
            except (OSError, UnicodeError):
                continue
            macro_literal_spans: list[tuple[int, int]] = []
            for match in _INCLUDE_RE.finditer(text):
                macro, literal = match.groups()
                macro_literal_spans.append(match.span(2))
                target_path = (source.parent / literal).resolve()
                finding = _finding_for_external_path(
                    repository=repository,
                    package_roots=package_roots,
                    consumer=consumer,
                    source=source,
                    target_path=target_path,
                    line=text.count("\n", 0, match.start()) + 1,
                    expression=f'{macro}!("{literal}")',
                    cross_package_kind=(
                        "private-source-include"
                        if macro == "include"
                        else "cross-package-include"
                    ),
                    repository_kind=(
                        "repository-private-source-include"
                        if macro == "include"
                        else "repository-external-include"
                    ),
                    external_kind=(
                        "external-private-source-include"
                        if macro == "include"
                        else "external-data-include"
                    ),
                )
                if finding is not None:
                    findings.append(finding)

            for match in _PATH_ATTRIBUTE_RE.finditer(text):
                literal = match.group(1)
                target_path = (source.parent / literal).resolve()
                finding = _finding_for_external_path(
                    repository=repository,
                    package_roots=package_roots,
                    consumer=consumer,
                    source=source,
                    target_path=target_path,
                    line=text.count("\n", 0, match.start()) + 1,
                    expression=f'#[path = "{literal}"]',
                    cross_package_kind="private-source-path-attribute",
                    repository_kind="repository-private-source-path-attribute",
                    external_kind="external-private-source-path-attribute",
                )
                if finding is not None:
                    findings.append(finding)

            for match in _SIBLING_SRC_LITERAL_RE.finditer(text):
                if any(
                    start <= match.start(1) and match.end(1) <= end
                    for start, end in macro_literal_spans
                ):
                    continue
                literal = match.group(1)
                target_path = (source.parent / literal).resolve()
                finding = _finding_for_external_path(
                    repository=repository,
                    package_roots=package_roots,
                    consumer=consumer,
                    source=source,
                    target_path=target_path,
                    line=text.count("\n", 0, match.start()) + 1,
                    expression=f'source path literal "{literal}"',
                    cross_package_kind="private-source-literal",
                    repository_kind="repository-private-source-literal",
                    external_kind="external-private-source-literal",
                )
                if finding is not None:
                    findings.append(finding)

    return sorted(
        findings,
        key=lambda item: (
            item.consumer,
            item.source_path,
            item.line,
            item.kind,
            item.target_path,
        ),
    )


def audit_couplings(
    findings: Sequence[CouplingFinding],
    allowances: Sequence[CouplingAllowance],
) -> SourceAudit:
    violations: list[Violation] = []
    allowance_by_key: dict[
        tuple[str, str, str, str, str], CouplingAllowance
    ] = {}
    malformed_keys: set[tuple[str, str, str, str, str]] = set()

    for allowance in allowances:
        missing = [
            field.name
            for field in dataclasses.fields(allowance)
            if not str(getattr(allowance, field.name)).strip()
        ]
        if missing:
            violations.append(
                Violation(
                    "MALFORMED_TRANSITIONAL_ALLOWANCE",
                    f"allowance {allowance.key!r} has empty fields {missing}",
                )
            )
            malformed_keys.add(allowance.key)
        if allowance.key in allowance_by_key:
            violations.append(
                Violation(
                    "DUPLICATE_TRANSITIONAL_ALLOWANCE",
                    f"duplicate allowance key {allowance.key!r}",
                )
            )
            malformed_keys.add(allowance.key)
        allowance_by_key[allowance.key] = allowance

    occurrences: collections.Counter[
        tuple[str, str, str, str, str]
    ] = collections.Counter(item.key for item in findings)
    for finding in findings:
        if finding.key not in allowance_by_key:
            violations.append(
                Violation(
                    "UNALLOWLISTED_PRIVATE_COUPLING",
                    (
                        f"{finding.kind}: {finding.consumer} {finding.source_path}:"
                        f"{finding.line} reaches {finding.target} "
                        f"{finding.target_path} via {finding.expression}"
                    ),
                )
            )

    allowed: list[tuple[CouplingAllowance, int]] = []
    for key, allowance in sorted(allowance_by_key.items()):
        count = occurrences[key]
        if count == 0:
            violations.append(
                Violation(
                    "STALE_OR_UNKNOWN_TRANSITIONAL_ALLOWANCE",
                    (
                        f"allowance {key!r} matches no current coupling; remove it "
                        "or correct the exact target"
                    ),
                )
            )
        elif key not in malformed_keys:
            allowed.append((allowance, count))

    return SourceAudit(tuple(findings), tuple(allowed), tuple(violations))


def _load_metadata(repository: Path, metadata_path: Path | None) -> dict[str, object]:
    if metadata_path is not None:
        with metadata_path.open("r", encoding="utf-8") as handle:
            loaded = json.load(handle)
        if not isinstance(loaded, dict):
            raise ValueError("metadata JSON root must be an object")
        return loaded

    command = ["cargo", "metadata", "--locked", "--format-version", "1"]
    completed = subprocess.run(
        command,
        cwd=repository,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(
            f"{' '.join(command)} failed with exit {completed.returncode}: {detail}"
        )
    loaded = json.loads(completed.stdout)
    if not isinstance(loaded, dict):
        raise ValueError("cargo metadata JSON root must be an object")
    return loaded


def _edge(
    package_id: str,
    *,
    kind: str | None = None,
    target: str | None = None,
) -> dict[str, object]:
    return {
        "name": package_id,
        "pkg": package_id,
        "dep_kinds": [{"kind": kind, "target": target}],
    }


def _fixture_metadata(*, negative: bool = False) -> dict[str, object]:
    names = (
        "nq-protocol",
        "nq-core",
        "nq-db",
        "nq-witness-api",
        "nq-monitor-agent",
        "nq-monitor",
    )
    packages = [
        {
            "id": name,
            "name": name,
            "manifest_path": f"/fixture/{name}/Cargo.toml",
            "source": None,
        }
        for name in names
    ]
    deps: dict[str, list[dict[str, object]]] = {
        "nq-protocol": [],
        "nq-core": [],
        "nq-db": [_edge("nq-core")],
        "nq-witness-api": [_edge("nq-core")],
        "nq-monitor-agent": [
            _edge("nq-core"),
            _edge("nq-witness-api"),
        ],
        "nq-monitor": [
            _edge("nq-core"),
            _edge("nq-db"),
            _edge("nq-monitor-agent"),
            _edge("nq-witness-api"),
        ],
    }
    if negative:
        # A target-qualified dev edge must count toward forbidden reachability,
        # and a target-qualified build edge must complete a detected cycle.
        deps["nq-monitor-agent"].append(
            _edge("nq-db", kind="dev", target="cfg(unix)")
        )
        deps["nq-db"].append(
            _edge("nq-monitor-agent", kind="build", target="cfg(windows)")
        )
    nodes = [{"id": name, "deps": deps[name]} for name in names]
    return {
        "packages": packages,
        "workspace_members": list(names),
        "workspace_root": "/fixture",
        "resolve": {"nodes": nodes},
    }


def run_negative_tripwire() -> None:
    _, violations = analyze_metadata(
        _fixture_metadata(negative=True), Path("/fixture")
    )
    codes = {item.code for item in violations}
    required = {"DEPENDENCY_CYCLE", "FORBIDDEN_DEPENDENCY"}
    if not required.issubset(codes):
        raise AssertionError(
            "negative fixture was not rejected for cycle and forbidden "
            f"target/dev/build reachability; saw {sorted(codes)}"
        )


class BoundarySelfTests(unittest.TestCase):
    def test_clean_metadata_fixture_passes(self) -> None:
        _, violations = analyze_metadata(_fixture_metadata(), Path("/fixture"))
        self.assertEqual([], violations)

    def test_negative_fixture_counts_dev_build_and_targets(self) -> None:
        graph, violations = analyze_metadata(
            _fixture_metadata(negative=True), Path("/fixture")
        )
        codes = {item.code for item in violations}
        self.assertIn("DEPENDENCY_CYCLE", codes)
        self.assertIn("FORBIDDEN_DEPENDENCY", codes)
        agent_id = _single_local_id(graph, "nq-monitor-agent")
        db_id = _single_local_id(graph, "nq-db")
        self.assertIsNotNone(agent_id)
        self.assertIsNotNone(db_id)
        info = graph.edges[(agent_id, db_id)]  # type: ignore[index]
        self.assertEqual({"dev"}, info.kinds)
        self.assertEqual({"cfg(unix)"}, info.targets)

    def test_dependency_allowance_is_exact_and_self_expiring(self) -> None:
        metadata = _fixture_metadata()
        metadata["packages"].append(  # type: ignore[union-attr]
            {
                "id": "nq",
                "name": "nq",
                "manifest_path": "/fixture/nq/Cargo.toml",
                "source": None,
            }
        )
        nodes = metadata["resolve"]["nodes"]  # type: ignore[index]
        nodes.append({"id": "nq", "deps": []})
        core = next(node for node in nodes if node["id"] == "nq-core")
        core["deps"].append(_edge("nq"))

        _, accepted = analyze_metadata(metadata, Path("/fixture"))
        self.assertEqual([], accepted)

        agent = next(node for node in nodes if node["id"] == "nq-monitor-agent")
        agent["deps"].append(_edge("nq"))
        _, changed = analyze_metadata(metadata, Path("/fixture"))
        codes = {item.code for item in changed}
        self.assertIn("FORBIDDEN_DEPENDENCY", codes)
        self.assertIn("STALE_OR_CHANGED_DEPENDENCY_ALLOWANCE", codes)

    def test_protocol_rejects_an_unlisted_external_dependency(self) -> None:
        metadata = _fixture_metadata()
        metadata["packages"].append(  # type: ignore[union-attr]
            {
                "id": "registry:anyhow",
                "name": "anyhow",
                "manifest_path": "/registry/anyhow/Cargo.toml",
                "source": "registry+https://example.invalid/index",
            }
        )
        nodes = metadata["resolve"]["nodes"]  # type: ignore[index]
        protocol = next(node for node in nodes if node["id"] == "nq-protocol")
        protocol["deps"].append(_edge("registry:anyhow"))
        _, violations = analyze_metadata(metadata, Path("/fixture"))
        self.assertIn(
            "FORBIDDEN_EXTERNAL_DEPENDENCY",
            {item.code for item in violations},
        )

    def test_cross_package_include_requires_exact_allowance(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            a = repo / "crates" / "nq-check-a"
            b = repo / "crates" / "nq-check-b"
            (a / "src").mkdir(parents=True)
            (b / "tests").mkdir(parents=True)
            (a / "Cargo.toml").write_text(
                '[package]\nname = "nq-check-a"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            (b / "Cargo.toml").write_text(
                '[package]\nname = "nq-check-b"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            (b / "tests" / "secret.json").write_text("{}\n", encoding="utf-8")
            source = a / "src" / "lib.rs"
            source.write_text(
                'const SECRET: &str = include_str!("../../nq-check-b/tests/secret.json");\n',
                encoding="utf-8",
            )
            roots = discover_package_roots(repo)
            findings = scan_source_couplings(repo, roots)
            self.assertEqual(1, len(findings))
            rejected = audit_couplings(findings, ())
            self.assertIn(
                "UNALLOWLISTED_PRIVATE_COUPLING",
                {item.code for item in rejected.violations},
            )
            finding = findings[0]
            allowance = CouplingAllowance(
                finding.kind,
                finding.consumer,
                finding.source_path,
                finding.target,
                finding.target_path,
                "fixture reason",
                "remove when fixture is package-local",
            )
            accepted = audit_couplings(findings, (allowance,))
            self.assertEqual((), accepted.violations)

            source.write_text("pub fn independent() {}\n", encoding="utf-8")
            stale = audit_couplings(
                scan_source_couplings(repo, roots), (allowance,)
            )
            self.assertIn(
                "STALE_OR_UNKNOWN_TRANSITIONAL_ALLOWANCE",
                {item.code for item in stale.violations},
            )

    def test_manifest_cannot_name_sibling_private_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            a = repo / "crates" / "nq-check-a"
            b = repo / "crates" / "nq-check-b"
            (a / "src").mkdir(parents=True)
            (b / "src").mkdir(parents=True)
            (a / "Cargo.toml").write_text(
                (
                    '[package]\nname = "nq-check-a"\nversion = "0.1.0"\n'
                    '[lib]\npath = "../nq-check-b/src/lib.rs"\n'
                ),
                encoding="utf-8",
            )
            (b / "Cargo.toml").write_text(
                '[package]\nname = "nq-check-b"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            (b / "src" / "lib.rs").write_text("", encoding="utf-8")
            findings = scan_source_couplings(
                repo, discover_package_roots(repo)
            )
            self.assertEqual(["manifest-private-path"], [item.kind for item in findings])

    def test_malformed_and_duplicate_allowances_fail(self) -> None:
        allowance = CouplingAllowance(
            "kind", "consumer", "source", "target", "target_path", "", "remove"
        )
        audit = audit_couplings((), (allowance, allowance))
        codes = {item.code for item in audit.violations}
        self.assertIn("MALFORMED_TRANSITIONAL_ALLOWANCE", codes)
        self.assertIn("DUPLICATE_TRANSITIONAL_ALLOWANCE", codes)
        self.assertIn("STALE_OR_UNKNOWN_TRANSITIONAL_ALLOWANCE", codes)


def _run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(BoundarySelfTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def _print_graph_summary(graph: GraphModel) -> None:
    print("resolved local package roles:")
    for package_id in sorted(
        graph.local_ids, key=lambda item: graph.packages[item].name
    ):
        package = graph.packages[package_id]
        print(f"  {package.name}: {role_for(package.name) or '<unknown>'}")
    print("resolved local dependency edges (normal/dev/build and all targets):")
    local_edges = [
        (source, target, info)
        for (source, target), info in graph.edges.items()
        if source in graph.local_ids and target in graph.local_ids
    ]
    if not local_edges:
        print("  <none>")
    for source, target, info in sorted(
        local_edges,
        key=lambda item: (
            graph.packages[item[0]].name,
            graph.packages[item[1]].name,
        ),
    ):
        print(
            f"  {graph.packages[source].name} -> "
            f"{graph.packages[target].name} ({info.render()})"
        )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="repository root (default: parent of scripts/)",
    )
    parser.add_argument(
        "--metadata",
        type=Path,
        help="read cargo metadata JSON from this file instead of invoking Cargo",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run fixture-driven unit tests without invoking Cargo",
    )
    args = parser.parse_args(argv)
    if args.self_test:
        return _run_self_tests()

    repository = args.repo.resolve()
    if not (repository / "Cargo.toml").is_file():
        print(f"FAIL: {repository} is not a Cargo repository root", file=sys.stderr)
        return 2

    try:
        run_negative_tripwire()
        print(
            "ok: negative fixture rejects target-qualified dev/build "
            "forbidden edges and cycles"
        )
        metadata = _load_metadata(repository, args.metadata)
        graph, graph_violations = analyze_metadata(metadata, repository)
        package_roots = discover_package_roots(repository)
        source_findings = scan_source_couplings(repository, package_roots)
        source_audit = audit_couplings(
            source_findings, TRANSITIONAL_COUPLING_ALLOWLIST
        )
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError) as error:
        print(f"FAIL: boundary gate could not inspect repository: {error}")
        return 1
    except AssertionError as error:
        print(f"FAIL: boundary gate self-check failed: {error}")
        return 1

    _print_graph_summary(graph)
    if not _cycle_violations(graph):
        print("ok: local dependency graph is acyclic across all edge kinds")
    if not _forbidden_dependency_violations(graph):
        print("ok: no forbidden role or exact-package reachability")
    if not _external_dependency_violations(graph):
        print("ok: constrained leaf external dependencies are bounded")
    if not _control_violations(graph):
        print("ok: positive control nq-monitor -[normal]-> nq-db is present")

    dependency_allowances = _active_dependency_allowances(graph)
    if dependency_allowances:
        print("transitional dependency allowances:")
        for allowance in dependency_allowances:
            print(f"  ALLOWED exact path: {' -> '.join(allowance.path)}")
            print(f"    reason: {allowance.reason}")
            print(f"    removal: {allowance.removal_condition}")
    else:
        print("ok: no transitional dependency allowances are active")

    if source_audit.allowed:
        print("transitional private coupling allowances:")
        for allowance, count in source_audit.allowed:
            print(
                f"  ALLOWED ({count} occurrence{'s' if count != 1 else ''}): "
                f"{allowance.kind} {allowance.consumer} "
                f"{allowance.source_path} -> {allowance.target_path}"
            )
            print(f"    reason: {allowance.reason}")
            print(f"    removal: {allowance.removal_condition}")
    else:
        print("ok: no transitional private coupling allowances are active")

    violations = list(graph_violations) + list(source_audit.violations)
    print("---")
    if violations:
        for violation in sorted(
            violations, key=lambda item: (item.code, item.message)
        ):
            print(f"FAIL [{violation.code}]: {violation.message}")
        print(f"CONSTELLATION BOUNDARY GATE: FAIL ({len(violations)} violation(s))")
        return 1
    print("CONSTELLATION BOUNDARY GATE: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
