#!/usr/bin/env python3
"""Exercise NQ installation and first use without a developer environment.

This program is evaluator instrumentation, not an installer. Product commands
run only against a supplied source archive or the release asset names declared
in docs/install/INSTALLATION_PROFILES.json. A failed installation is retained
as evidence; the program never substitutes a checkout, cache, binary, port, or
configuration merely to obtain a passing run.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import signal
import socket
import sqlite3
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Sequence


CAMPAIGN_SCHEMA = "nq.install_first_run.campaign.v1"
STEP_SCHEMA = "nq.install_first_run.step.v1"
DEFAULT_PATH = "/usr/local/bin:/usr/bin:/bin"
DEFAULT_RELEASE_BASE = "https://github.com/unpingable/nq/releases/latest/download"
ARCH_NAMES = {"x86_64": "amd64", "aarch64": "arm64", "arm64": "arm64"}


@dataclass(frozen=True)
class StepResult:
    schema: str
    step_id: str
    description: str
    argv: list[str]
    cwd: str
    started_at: str
    finished_at: str
    duration_ms: int
    exit_code: int | None
    timed_out: bool
    stdin: str
    stdout: str
    stderr: str


def utc_now() -> str:
    return (
        datetime.now(timezone.utc)
        .isoformat(timespec="milliseconds")
        .replace("+00:00", "Z")
    )


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


class Campaign:
    def __init__(self, arguments: argparse.Namespace) -> None:
        self.args = arguments
        self.output = arguments.output.resolve()
        if self.output.exists():
            raise ValueError(f"output already exists: {self.output}")
        self.output.mkdir(parents=True)
        (self.output / "steps").mkdir()
        self.workspace = Path(
            tempfile.mkdtemp(prefix="nq-first-run-", dir="/tmp")
        ).resolve()
        self.home = self.workspace / "home"
        self.work = self.workspace / "work"
        self.install_root = self.workspace / "install"
        self.bin_dir = self.install_root / "bin"
        self.config_dir = self.install_root / "config"
        self.state_dir = self.install_root / "state"
        self.tmp_dir = self.workspace / "tmp"
        for path in (
            self.home,
            self.work,
            self.bin_dir,
            self.config_dir,
            self.state_dir,
            self.tmp_dir,
        ):
            path.mkdir(parents=True)
        self.clean_env = {
            "HOME": str(self.home),
            "XDG_CONFIG_HOME": str(self.home / ".config"),
            "XDG_CACHE_HOME": str(self.home / ".cache"),
            "CARGO_HOME": str(self.home / ".cargo"),
            "RUSTUP_HOME": str(self.home / ".rustup"),
            "TMPDIR": str(self.tmp_dir),
            "PATH": f"{self.bin_dir}:{DEFAULT_PATH}",
            "LANG": "C",
            "LC_ALL": "C",
            "TZ": "UTC",
            "GIT_TERMINAL_PROMPT": "0",
            "PYTHONDONTWRITEBYTECODE": "1",
            "NO_PROXY": "127.0.0.1,localhost",
        }
        if arguments.dependency_mode == "isolated-offline":
            self.clean_env["CARGO_NET_OFFLINE"] = "true"
            self.clean_env["RUSTUP_DIST_SERVER"] = (
                "http://127.0.0.1:9/offline-rustup-disabled"
            )
            self.clean_env["RUSTUP_UPDATE_ROOT"] = (
                "http://127.0.0.1:9/offline-rustup-disabled"
            )
        self.started_at = utc_now()
        self.started_ns = time.monotonic_ns()
        self.steps: list[StepResult] = []
        self.observations: dict[str, Any] = {}
        self.blocker: dict[str, Any] | None = None
        self.profile_result_at_ms: int | None = None
        self.host_result_at_ms: int | None = None
        self.source_root: Path | None = None
        self.profile: dict[str, Any] | None = None
        self.installed_binaries: dict[str, Path] = {}

    def record_environment(self) -> None:
        command_paths = {
            command: shutil.which(command, path=DEFAULT_PATH)
            for command in (
                "bash",
                "cargo",
                "curl",
                "git",
                "install",
                "rustc",
                "sha256sum",
                "tar",
            )
        }
        write_json(
            self.output / "environment.json",
            {
                "schema": CAMPAIGN_SCHEMA,
                "captured_at": utc_now(),
                "platform": {
                    "system": platform.system(),
                    "release": platform.release(),
                    "machine": platform.machine(),
                    "python": platform.python_version(),
                    "effective_uid": os.geteuid(),
                    "effective_gid": os.getegid(),
                },
                "effective_environment": self.clean_env,
                "inherited_environment_variable_count": 0,
                "visible_commands": command_paths,
                "network_policy": self.args.dependency_mode,
                "workspace": str(self.workspace),
                "workspace_isolation": {
                    "empty_home": True,
                    "empty_cargo_home": True,
                    "empty_rustup_home": True,
                    "sibling_checkout_search_roots": [str(self.workspace)],
                    "developer_target_reuse": False,
                    "credential_or_proxy_inheritance": False,
                },
            },
        )

    def run_step(
        self,
        step_id: str,
        description: str,
        argv: Sequence[str],
        *,
        cwd: Path,
        timeout_s: float = 300.0,
        env: dict[str, str] | None = None,
    ) -> StepResult:
        step_dir = self.output / "steps" / step_id
        if step_dir.exists():
            raise RuntimeError(f"duplicate step id: {step_id}")
        step_dir.mkdir()
        command = [str(part) for part in argv]
        effective_env = dict(self.clean_env if env is None else env)
        write_json(
            step_dir / "invocation.json",
            {
                "schema": STEP_SCHEMA,
                "description": description,
                "argv": command,
                "cwd": str(cwd),
                "environment": effective_env,
                "stdin": "null_device",
                "timeout_ms": int(timeout_s * 1000),
            },
        )
        started_at = utc_now()
        started_ns = time.monotonic_ns()
        timed_out = False
        exit_code: int | None
        stdout = ""
        stderr = ""
        try:
            completed = subprocess.run(
                command,
                cwd=cwd,
                env=effective_env,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                check=False,
                timeout=timeout_s,
                start_new_session=True,
            )
            exit_code = completed.returncode
            stdout = completed.stdout
            stderr = completed.stderr
        except subprocess.TimeoutExpired as error:
            timed_out = True
            exit_code = None
            stdout = (error.stdout or "") if isinstance(error.stdout, str) else ""
            stderr = (error.stderr or "") if isinstance(error.stderr, str) else ""
            stderr += (
                f"\nevaluator timeout after {timeout_s:.3f}s; "
                "no prompt response or environment repair was supplied\n"
            )
        except OSError as error:
            exit_code = 127 if error.errno == 2 else 126
            stderr = f"{error.__class__.__name__}: {error}\n"
        finished_ns = time.monotonic_ns()
        finished_at = utc_now()
        (step_dir / "stdout.log").write_text(stdout, encoding="utf-8")
        (step_dir / "stderr.log").write_text(stderr, encoding="utf-8")
        result = StepResult(
            schema=STEP_SCHEMA,
            step_id=step_id,
            description=description,
            argv=command,
            cwd=str(cwd),
            started_at=started_at,
            finished_at=finished_at,
            duration_ms=(finished_ns - started_ns) // 1_000_000,
            exit_code=exit_code,
            timed_out=timed_out,
            stdin="null_device",
            stdout=str((step_dir / "stdout.log").relative_to(self.output)),
            stderr=str((step_dir / "stderr.log").relative_to(self.output)),
        )
        write_json(step_dir / "result.json", asdict(result))
        self.steps.append(result)
        return result

    def note_step(self, step_id: str, value: dict[str, Any]) -> None:
        step_dir = self.output / "steps" / step_id
        if step_dir.exists():
            raise RuntimeError(f"duplicate step id: {step_id}")
        step_dir.mkdir()
        write_json(
            step_dir / "result.json",
            {
                "schema": STEP_SCHEMA,
                "step_id": step_id,
                "duration_ms": 0,
                **value,
            },
        )

    def block(self, phase: str, code: str, detail: str) -> None:
        if self.blocker is None:
            self.blocker = {"phase": phase, "code": code, "detail": detail}

    def load_profiles(self, path: Path, basis: str) -> bool:
        path = path.resolve()
        try:
            contract = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            self.block("discovery", "installation_profile_unreadable", str(error))
            return False
        if contract.get("schema") != "nq.installation_profiles.v1":
            self.block(
                "discovery",
                "unsupported_installation_profile_schema",
                f"found {contract.get('schema')!r}",
            )
            return False
        matches = [
            profile
            for profile in contract.get("profiles", [])
            if profile.get("id") == self.args.profile
        ]
        if len(matches) != 1:
            self.block(
                "discovery",
                "unknown_or_duplicate_profile",
                self.args.profile,
            )
            return False
        self.profile = matches[0]
        self.observations["discovery"] = {
            "status": "completed",
            "profile_contract": "docs/install/INSTALLATION_PROFILES.json",
            "profile_contract_basis": basis,
            "profile_contract_sha256": sha256(path),
            "owner_repository": contract["product"]["owner_repository"],
            "owner_package": self.profile["owner_package"],
            "required_at_runtime": self.profile["required_at_runtime"],
            "optional_components": self.profile["optional_components"],
            "profile_status": self.profile["status"],
            "purpose": self.profile["purpose"],
            "first_use_limit": self.profile["first_use_limit"],
        }
        return True

    def exercise_missing_dependency(self) -> None:
        empty_path = self.workspace / "empty-path"
        empty_path.mkdir()
        env = dict(self.clean_env)
        env["PATH"] = str(empty_path)
        result = self.run_step(
            "005-missing-build-dependency",
            (
                "Exercise a missing Cargo prerequisite without installing it "
                "or falling back to the evaluator's PATH"
            ),
            ["/usr/bin/env", "cargo", "--version"],
            cwd=self.work,
            timeout_s=10,
            env=env,
        )
        stderr = read_text(self.output / result.stderr)
        self.observations["missing_dependency"] = {
            "status": "observed" if result.exit_code != 0 else "unexpected_success",
            "exit_code": result.exit_code,
            "product_installer_preflight_available": False,
            "actionability": (
                "The operating system names the missing command. NQ ships no "
                "installer preflight that enumerates source-build prerequisites."
            ),
            "message_present": bool(stderr.strip()),
        }

    def prepare_source_archive(self) -> bool:
        archive = self.args.source_archive
        if archive is None:
            self.block(
                "installation",
                "source_archive_required",
                "--source-archive is required for the source-archive track",
            )
            return False
        archive = archive.resolve()
        if not archive.is_file():
            self.block(
                "installation",
                "source_archive_missing",
                str(archive),
            )
            return False

        try:
            inspection = inspect_source_archive(archive)
        except (OSError, tarfile.TarError, ValueError) as error:
            self.block("installation", "source_archive_refused", str(error))
            return False
        self.note_step(
            "010-source-archive-inspection",
            {
                "description": (
                    "Validate the immutable source archive before extraction; "
                    "links, devices, unsafe paths, and checkout metadata are refused"
                ),
                "exit_code": 0,
                "archive": str(archive),
                "sha256": sha256(archive),
                **inspection,
            },
        )
        extract_root = self.work / "source"
        extract_root.mkdir()
        started_ns = time.monotonic_ns()
        with tarfile.open(archive, "r:*") as source:
            source.extractall(extract_root)
        candidates = [
            path.parent
            for path in extract_root.rglob("Cargo.toml")
            if len(path.relative_to(extract_root).parts) <= 2
        ]
        if len(candidates) != 1:
            self.block(
                "installation",
                "archive_workspace_root_ambiguous",
                f"candidate Cargo workspaces: {[str(path) for path in candidates]}",
            )
            return False
        self.source_root = candidates[0].resolve()
        self.note_step(
            "011-source-archive-extraction",
            {
                "description": "Extract the accepted source archive into the isolated workspace",
                "exit_code": 0,
                "duration_ms": (time.monotonic_ns() - started_ns) // 1_000_000,
                "source_root": str(self.source_root),
                "git_checkout_present": (self.source_root / ".git").exists(),
                "sibling_checkout_present": False,
            },
        )
        return True

    def install_source_profile(self) -> bool:
        assert self.profile is not None
        assert self.source_root is not None
        source = self.profile["source"]
        cargo = shutil.which("cargo", path=DEFAULT_PATH)
        if cargo is None:
            self.block(
                "installation",
                "cargo_missing",
                "Cargo is not visible on the documented clean PATH",
            )
            return False
        build = self.run_step(
            "020-source-build",
            (
                f"Build the {self.args.profile} profile exactly as declared by "
                "the versioned installation profile"
            ),
            [cargo, *source["cargo_args"]],
            cwd=self.source_root,
            timeout_s=self.args.build_timeout,
        )
        if build.exit_code != 0:
            self.block(
                "installation",
                "source_build_failed",
                f"step {build.step_id} exited {build.exit_code}",
            )
            return False

        for index, binary in enumerate(source["binaries"]):
            source_binary = self.source_root / binary["source"]
            destination = self.bin_dir / binary["name"]
            result = self.run_step(
                f"02{index + 1}-install-{binary['name']}",
                (
                    f"Install only {binary['name']} into the isolated profile "
                    "prefix; no system directory or sibling package is used"
                ),
                [
                    "/usr/bin/install",
                    "-m",
                    "0755",
                    str(source_binary),
                    str(destination),
                ],
                cwd=self.source_root,
                timeout_s=30,
            )
            if result.exit_code != 0:
                self.block(
                    "installation",
                    "binary_install_failed",
                    binary["name"],
                )
                return False
            self.installed_binaries[binary["name"]] = destination

        metadata = self.run_step(
            "029-source-package-metadata",
            (
                "Record workspace package and binary identities after the "
                "build; path dependencies must remain inside the extracted archive"
            ),
            [cargo, "metadata", "--format-version", "1", "--no-deps", "--locked"],
            cwd=self.source_root,
            timeout_s=60,
        )
        self.analyze_metadata(metadata)
        self.record_versions()
        return True

    def install_release_profile(self) -> bool:
        assert self.profile is not None
        release = self.profile.get("release")
        if release is None:
            self.block(
                "installation",
                "no_documented_release_artifact",
                (
                    f"profile {self.args.profile} has no release package; "
                    "the harness will not invent an asset name"
                ),
            )
            return False
        architecture = ARCH_NAMES.get(platform.machine())
        if architecture is None:
            self.block(
                "installation",
                "unsupported_release_architecture",
                platform.machine(),
            )
            return False
        base = self.args.release_base.rstrip("/")
        curl = shutil.which("curl", path=DEFAULT_PATH)
        checksum = shutil.which("sha256sum", path=DEFAULT_PATH)
        if curl is None or checksum is None:
            self.block(
                "installation",
                "release_dependency_missing",
                f"curl={curl!r}, sha256sum={checksum!r}",
            )
            return False
        stage = self.work / "release"
        stage.mkdir()
        for index, asset_template in enumerate(release["assets"]):
            asset = asset_template.format(arch=architecture)
            binary_name = asset.split("-linux-", 1)[0]
            downloaded = stage / asset
            digest_file = stage / f"{asset}.sha256"
            fetch = self.run_step(
                f"01{index}a-download-{binary_name}",
                "Download the exact release asset declared by the installation profile",
                [curl, "-fL", f"{base}/{asset}", "-o", str(downloaded)],
                cwd=stage,
                timeout_s=self.args.download_timeout,
            )
            if fetch.exit_code != 0:
                self.block(
                    "installation",
                    "release_asset_unavailable",
                    f"{base}/{asset}",
                )
                return False
            digest = self.run_step(
                f"01{index}b-download-{binary_name}-checksum",
                "Download the checksum paired with the exact release asset",
                [curl, "-fL", f"{base}/{asset}.sha256", "-o", str(digest_file)],
                cwd=stage,
                timeout_s=self.args.download_timeout,
            )
            if digest.exit_code != 0:
                self.block(
                    "installation",
                    "release_checksum_unavailable",
                    f"{base}/{asset}.sha256",
                )
                return False
            verified = self.run_step(
                f"01{index}c-verify-{binary_name}",
                "Verify the downloaded bytes before installing the binary",
                [checksum, "--check", digest_file.name],
                cwd=stage,
                timeout_s=30,
            )
            if verified.exit_code != 0:
                self.block(
                    "installation",
                    "release_checksum_mismatch",
                    asset,
                )
                return False
            destination = self.bin_dir / binary_name
            installed = self.run_step(
                f"01{index}d-install-{binary_name}",
                "Install the verified release binary into the isolated prefix",
                [
                    "/usr/bin/install",
                    "-m",
                    "0755",
                    str(downloaded),
                    str(destination),
                ],
                cwd=stage,
                timeout_s=30,
            )
            if installed.exit_code != 0:
                self.block("installation", "binary_install_failed", binary_name)
                return False
            self.installed_binaries[binary_name] = destination
        self.record_versions()
        return True

    def analyze_metadata(self, result: StepResult) -> None:
        if result.exit_code != 0:
            self.observations["environment_leaks"] = {
                "status": "indeterminate",
                "reason": "cargo metadata failed",
            }
            return
        try:
            metadata = json.loads(read_text(self.output / result.stdout))
        except json.JSONDecodeError as error:
            self.observations["environment_leaks"] = {
                "status": "indeterminate",
                "reason": f"cargo metadata output invalid: {error}",
            }
            return
        assert self.source_root is not None
        outside = []
        for package in metadata.get("packages", []):
            manifest = Path(package["manifest_path"]).resolve()
            if not manifest.is_relative_to(self.source_root):
                outside.append(str(manifest))
        self.observations["environment_leaks"] = {
            "status": "not_detected" if not outside else "detected",
            "path_dependencies_outside_source_archive": outside,
            "inherited_environment_variable_count": 0,
            "sibling_checkout_used": False,
            "developer_target_reused": False,
        }

    def record_versions(self) -> None:
        versions: dict[str, Any] = {}
        for index, (name, binary) in enumerate(sorted(self.installed_binaries.items())):
            result = self.run_step(
                f"030-version-{index:02d}-{name}",
                f"Record the installed {name} version using its own CLI",
                [str(binary), "--version"],
                cwd=self.install_root,
                timeout_s=15,
            )
            versions[name] = {
                "exit_code": result.exit_code,
                "reported": read_text(self.output / result.stdout).strip(),
                "binary_sha256": sha256(binary) if binary.is_file() else None,
            }
        self.observations["installed_versions"] = versions

    def copy_config(self, source_relative: str, destination_name: str) -> Path | None:
        if self.source_root is None:
            self.block(
                "configuration",
                "configuration_not_packaged_with_release",
                (
                    f"{source_relative} is required but this release profile "
                    "does not include a configuration bundle"
                ),
            )
            return None
        source = self.source_root / source_relative
        destination = self.config_dir / destination_name
        result = self.run_step(
            f"040-copy-{destination_name.replace('.', '-')}",
            (
                f"Copy the literal packaged configuration {source_relative}; "
                "no JSON is inferred or repaired"
            ),
            ["/usr/bin/install", "-m", "0600", str(source), str(destination)],
            cwd=self.source_root,
            timeout_s=30,
        )
        if result.exit_code != 0:
            self.block("configuration", "packaged_configuration_missing", source_relative)
            return None
        return destination

    def first_use(self) -> None:
        assert self.profile is not None
        kind = self.profile["first_use"]
        if kind == "suite_plan":
            self.first_use_suite()
        elif kind == "witness_validation":
            self.first_use_witness()
        elif kind == "monitor_surface":
            self.first_use_monitor()
        elif kind == "legacy_operational":
            self.first_use_legacy()
        else:
            self.block("first_use", "unsupported_first_use_kind", str(kind))

    def first_use_suite(self) -> None:
        binary = self.installed_binaries["nq-suite"]
        config = self.copy_config(
            "crates/nq-suite/examples/minimal-public.json", "nq-suite.json"
        )
        if config is None:
            return
        validation = self.run_step(
            "050-suite-config-validation",
            "Validate the packaged minimal host-only suite configuration",
            [str(binary), "config", "validate", "--config", str(config)],
            cwd=self.install_root,
            timeout_s=30,
        )
        if validation.exit_code != 0:
            self.block("configuration", "suite_configuration_refused", str(config))
            return
        plan = self.run_step(
            "051-suite-plan",
            "Emit the deterministic composition plan without launching checks",
            [str(binary), "plan", "--config", str(config), "--pretty"],
            cwd=self.install_root,
            timeout_s=30,
        )
        if plan.exit_code != 0:
            self.block("first_use", "suite_plan_failed", str(config))
            return
        try:
            document = json.loads(read_text(self.output / plan.stdout))
        except json.JSONDecodeError as error:
            self.block("first_use", "suite_plan_not_json", str(error))
            return
        enabled = [
            entry.get("pack_id") for entry in document.get("enabled_packs", [])
        ]
        launch = document.get("launch", {})
        exact = enabled == ["nq.host"]
        self.observations["first_use"] = {
            "status": "profile_result",
            "kind": "composition_plan",
            "enabled_packs": enabled,
            "conservative_host_only": exact,
            "launch_available": launch.get("available"),
            "host_observation_produced": False,
            "unknown_preserved": (
                "No check ran; validation and planning do not establish an observation."
            ),
        }
        if not exact:
            self.block(
                "first_use",
                "minimal_profile_not_host_only",
                repr(enabled),
            )
            return
        self.profile_result_at_ms = self.elapsed_ms()

    def first_use_witness(self) -> None:
        binary = self.installed_binaries["nq-witness-tool"]
        fixture = self.copy_config(
            "crates/nq-witness/tests/fixtures/zab2nq-external-projection.json",
            "external-projection.json",
        )
        if fixture is None:
            return
        result = self.run_step(
            "050-validate-witness-artifact",
            "Validate one packaged external-projection witness through the public artifact tool",
            [str(binary), "validate-packet", str(fixture)],
            cwd=self.install_root,
            timeout_s=30,
        )
        if result.exit_code != 0:
            self.block("first_use", "witness_validation_failed", str(fixture))
            return
        try:
            document = json.loads(read_text(self.output / result.stdout))
        except json.JSONDecodeError as error:
            self.block("first_use", "witness_result_not_json", str(error))
            return
        self.observations["first_use"] = {
            "status": "profile_result",
            "kind": "witness_artifact_validation",
            "tool_status": document.get("status"),
            "host_observation_produced": False,
            "authority_limit": document.get("authority"),
        }
        self.profile_result_at_ms = self.elapsed_ms()

    def first_use_monitor(self) -> None:
        binary = self.installed_binaries["nq-monitor"]
        packaged = self.copy_config(
            "deploy/quickstart/monitor-only.json", "aggregator.json"
        )
        if packaged is None:
            return
        config = self.rebase_config(packaged, monitor_only=True)
        validation = self.run_step(
            "050-monitor-config-validation",
            "Validate the packaged monitor-only configuration without opening state",
            [str(binary), "config", "validate", "--config", str(config)],
            cwd=self.install_root,
            timeout_s=30,
        )
        if validation.exit_code != 0:
            self.block("configuration", "monitor_configuration_refused", str(config))
            return
        process = self.start_process(
            "051-monitor-process",
            "Start only the central monitor and dashboard with no sources",
            [str(binary), "serve", "--config", str(config)],
            cwd=self.install_root,
        )
        try:
            if not wait_http("http://127.0.0.1:9848/api/overview", process, 30):
                self.block(
                    "first_use",
                    "monitor_http_not_ready",
                    "127.0.0.1:9848",
                )
                return
            overview = self.run_step(
                "052-monitor-overview",
                "Read the monitor-only overview; an empty queue is not host health",
                ["/usr/bin/curl", "-fsS", "http://127.0.0.1:9848/api/overview"],
                cwd=self.install_root,
                timeout_s=15,
            )
            if overview.exit_code != 0:
                self.block("first_use", "monitor_overview_failed", "curl")
                return
            self.observations["first_use"] = {
                "status": "profile_result",
                "kind": "monitor_surface",
                "configured_source_count": 0,
                "host_observation_produced": False,
                "unknown_preserved": (
                    "No sources were configured; the running dashboard is not "
                    "evidence that any monitored system is healthy."
                ),
            }
            self.profile_result_at_ms = self.elapsed_ms()
        finally:
            self.finish_process("051-monitor-process", process)

    def first_use_legacy(self) -> None:
        witness_binary = self.installed_binaries["nq-witness"]
        monitor_binary = self.installed_binaries["nq-monitor"]
        publisher = self.copy_config(
            "deploy/quickstart/publisher.json", "publisher.json"
        )
        aggregator = self.copy_config(
            "deploy/quickstart/aggregator.json", "aggregator.json"
        )
        if publisher is None or aggregator is None:
            return
        aggregator = self.rebase_config(aggregator)
        witness_validation = self.run_step(
            "050-witness-config-validation",
            "Validate the literal publisher configuration with no checks or listener",
            [
                str(witness_binary),
                "config",
                "validate",
                "--config",
                str(publisher),
            ],
            cwd=self.install_root,
            timeout_s=30,
        )
        monitor_validation = self.run_step(
            "051-monitor-config-validation",
            "Validate the literal aggregator configuration without opening the database",
            [
                str(monitor_binary),
                "config",
                "validate",
                "--config",
                str(aggregator),
            ],
            cwd=self.install_root,
            timeout_s=30,
        )
        if witness_validation.exit_code != 0 or monitor_validation.exit_code != 0:
            self.block(
                "configuration",
                "literal_quickstart_configuration_refused",
                "publisher or aggregator",
            )
            return
        witness = self.start_process(
            "052-witness-process",
            "Start the compatibility local host publisher on the documented loopback port",
            [str(witness_binary), "--config", str(publisher)],
            cwd=self.install_root,
        )
        monitor: subprocess.Popen[str] | None = None
        try:
            if not wait_http("http://127.0.0.1:9847/state", witness, 30):
                self.block(
                    "first_use",
                    "witness_http_not_ready",
                    "The literal port may be occupied; it was not changed.",
                )
                return
            state = self.run_step(
                "053-witness-state",
                "Read the documented local witness state",
                ["/usr/bin/curl", "-fsS", "http://127.0.0.1:9847/state"],
                cwd=self.install_root,
                timeout_s=15,
            )
            if state.exit_code != 0:
                self.block("first_use", "witness_state_failed", "curl")
                return
            monitor = self.start_process(
                "054-monitor-process",
                "Start the monitor on the documented loopback port",
                [str(monitor_binary), "serve", "--config", str(aggregator)],
                cwd=self.install_root,
            )
            if not wait_http("http://127.0.0.1:9848/api/overview", monitor, 30):
                self.block(
                    "first_use",
                    "monitor_http_not_ready",
                    "The literal port may be occupied; it was not changed.",
                )
                return
            overview = self.run_step(
                "055-overview",
                "Read the dashboard overview after HTTP readiness",
                ["/usr/bin/curl", "-fsS", "http://127.0.0.1:9848/api/overview"],
                cwd=self.install_root,
                timeout_s=15,
            )
            query_argv = [
                str(monitor_binary),
                "query",
                "--remote",
                "http://127.0.0.1:9848",
                (
                    "SELECT host, cpu_load_1m, mem_pressure_pct, "
                    "disk_used_pct, age_s FROM v_hosts"
                ),
            ]
            query = self.run_step(
                "056-v-hosts",
                "Query for the first meaningful local host observation",
                query_argv,
                cwd=self.install_root,
                timeout_s=30,
            )
            output = read_text(self.output / query.stdout)
            if query.exit_code == 0 and "local-host" not in output:
                wait = self.run_step(
                    "057-one-observation-interval",
                    "Wait one documented observation interval without changing configuration",
                    ["/usr/bin/sleep", "10"],
                    cwd=self.install_root,
                    timeout_s=15,
                )
                if wait.exit_code == 0:
                    query = self.run_step(
                        "058-v-hosts-after-interval",
                        "Repeat the documented host query after one interval",
                        query_argv,
                        cwd=self.install_root,
                        timeout_s=30,
                    )
                    output = read_text(self.output / query.stdout)
            meaningful = (
                overview.exit_code == 0
                and query.exit_code == 0
                and "local-host" in output
            )
            self.observations["first_use"] = {
                "status": "profile_result" if meaningful else "blocked",
                "kind": "monitored_host_observation",
                "host_observation_produced": meaningful,
                "host_identity": "local-host" if meaningful else None,
            }
            if meaningful:
                self.profile_result_at_ms = self.elapsed_ms()
                self.host_result_at_ms = self.profile_result_at_ms
            else:
                self.block(
                    "first_use",
                    "host_observation_not_reached",
                    "The query did not contain local-host.",
                )
        finally:
            if monitor is not None:
                self.finish_process("054-monitor-process", monitor)
            self.finish_process("052-witness-process", witness)

    def rebase_config(self, path: Path, monitor_only: bool = False) -> Path:
        document = json.loads(path.read_text(encoding="utf-8"))
        document["db_path"] = str(self.state_dir / "nq.db")
        if "liveness" in document:
            document["liveness"]["path"] = str(self.state_dir / "liveness.json")
        if monitor_only:
            document["sources"] = []
        rebased = self.config_dir / f"runtime-{path.name}"
        write_json(rebased, document)
        self.note_step(
            f"041-rebase-{path.name.replace('.', '-')}",
            {
                "description": (
                    "Replace only relative state-output paths with isolated "
                    "workspace paths; semantic configuration and documented ports remain unchanged"
                ),
                "exit_code": 0,
                "source": str(path),
                "destination": str(rebased),
                "changed_fields": ["db_path", "liveness.path"],
                "operator_assistance": True,
            },
        )
        return rebased

    def start_process(
        self,
        step_id: str,
        description: str,
        argv: Sequence[str],
        *,
        cwd: Path,
    ) -> subprocess.Popen[str]:
        step_dir = self.output / "steps" / step_id
        if step_dir.exists():
            raise RuntimeError(f"duplicate step id: {step_id}")
        step_dir.mkdir()
        command = [str(part) for part in argv]
        write_json(
            step_dir / "invocation.json",
            {
                "schema": STEP_SCHEMA,
                "description": description,
                "argv": command,
                "cwd": str(cwd),
                "environment": self.clean_env,
                "stdin": "null_device",
                "managed_process": True,
            },
        )
        stdout = (step_dir / "stdout.log").open("w", encoding="utf-8")
        stderr = (step_dir / "stderr.log").open("w", encoding="utf-8")
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=self.clean_env,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            text=True,
            start_new_session=True,
        )
        process._nq_evidence = {  # type: ignore[attr-defined]
            "step_id": step_id,
            "description": description,
            "argv": command,
            "cwd": str(cwd),
            "started_at": utc_now(),
            "started_ns": time.monotonic_ns(),
            "stdout_handle": stdout,
            "stderr_handle": stderr,
        }
        return process

    def finish_process(
        self, step_id: str, process: subprocess.Popen[str]
    ) -> None:
        metadata = process._nq_evidence  # type: ignore[attr-defined]
        if metadata["step_id"] != step_id:
            raise RuntimeError("managed process step identity mismatch")
        was_running = process.poll() is None
        terminate(process)
        metadata["stdout_handle"].close()
        metadata["stderr_handle"].close()
        result = StepResult(
            schema=STEP_SCHEMA,
            step_id=step_id,
            description=metadata["description"],
            argv=metadata["argv"],
            cwd=metadata["cwd"],
            started_at=metadata["started_at"],
            finished_at=utc_now(),
            duration_ms=(time.monotonic_ns() - metadata["started_ns"]) // 1_000_000,
            exit_code=process.poll(),
            timed_out=False,
            stdin="null_device",
            stdout=f"steps/{step_id}/stdout.log",
            stderr=f"steps/{step_id}/stderr.log",
        )
        write_json(self.output / "steps" / step_id / "result.json", asdict(result))
        write_json(
            self.output / "steps" / step_id / "lifecycle.json",
            {
                "terminated_by_harness": was_running,
                "process_group_isolated": True,
            },
        )
        self.steps.append(result)

    def failure_matrix(self) -> None:
        results: dict[str, Any] = {}
        primary = self.primary_config_binary()
        if primary is not None:
            name, binary, prefix = primary
            missing = self.run_step(
                "100-wrong-config-path",
                "Exercise a configuration path that does not exist",
                [str(binary), *prefix, str(self.config_dir / "missing.json")],
                cwd=self.install_root,
                timeout_s=15,
            )
            results["wrong_path"] = self.classify_failure(
                missing,
                expected=("cannot read", "no state was changed"),
            )

            malformed_path = self.config_dir / "malformed.json"
            malformed_path.write_text('{"unexpected": true}\n', encoding="utf-8")
            malformed = self.run_step(
                "101-malformed-configuration",
                "Exercise a syntactically valid but structurally unknown configuration",
                [str(binary), *prefix, str(malformed_path)],
                cwd=self.install_root,
                timeout_s=15,
            )
            results["malformed_configuration"] = self.classify_failure(
                malformed,
                expected=malformed_config_safety_fragments(name),
            )

            if os.geteuid() == 0:
                results["permission_failure"] = {
                    "status": "not_applicable",
                    "reason": (
                        "The evaluator is root; mode 000 would not reproduce an "
                        "ordinary operator permission denial."
                    ),
                }
            else:
                denied = self.config_dir / "permission-denied.json"
                denied.write_text("{}\n", encoding="utf-8")
                denied.chmod(0)
                try:
                    permission = self.run_step(
                        "102-permission-denied-config",
                        "Exercise an unreadable configuration without changing ownership",
                        [str(binary), *prefix, str(denied)],
                        cwd=self.install_root,
                        timeout_s=15,
                    )
                finally:
                    denied.chmod(0o600)
                results["permission_failure"] = self.classify_failure(
                    permission,
                    expected=("cannot read", "no state"),
                )

        if "nq-suite" in self.installed_binaries:
            results.update(self.suite_failure_scenarios())
        if "nq-witness-tool" in self.installed_binaries:
            results.update(self.witness_tool_failure_scenarios())
        if "nq-witness" in self.installed_binaries:
            results["occupied_witness_port"] = self.occupied_witness_scenario()
        if "nq-monitor" in self.installed_binaries:
            results["occupied_monitor_port"] = self.occupied_monitor_scenario()
            results["unavailable_sibling_service"] = (
                self.unavailable_source_scenario()
            )
            results["stale_database"] = self.stale_database_scenario()
        else:
            results["stale_database"] = {
                "status": "not_applicable",
                "reason": "This profile does not install nq-monitor.",
            }
            results["unavailable_sibling_service"] = {
                "status": "not_applicable",
                "reason": "This profile does not install nq-monitor.",
            }
        self.observations["failure_and_recovery"] = results
        write_json(self.output / "failure-matrix.json", results)

    def witness_tool_failure_scenarios(self) -> dict[str, Any]:
        binary = self.installed_binaries["nq-witness-tool"]
        missing = self.run_step(
            "100-wrong-witness-path",
            "Exercise a witness artifact path that does not exist",
            [
                str(binary),
                "validate-packet",
                str(self.config_dir / "missing-witness.json"),
            ],
            cwd=self.install_root,
            timeout_s=15,
        )
        malformed_path = self.config_dir / "malformed-witness.json"
        malformed_path.write_text("{not JSON\n", encoding="utf-8")
        malformed = self.run_step(
            "101-malformed-witness",
            "Exercise malformed witness JSON without substituting another artifact",
            [str(binary), "validate-packet", str(malformed_path)],
            cwd=self.install_root,
            timeout_s=15,
        )
        return {
            "wrong_path": self.classify_structured_refusal(
                missing, expected_code="witness.tool.io_error"
            ),
            "malformed_configuration": self.classify_structured_refusal(
                malformed, expected_code="witness.malformed_json"
            ),
        }

    def classify_structured_refusal(
        self, result: StepResult, *, expected_code: str
    ) -> dict[str, Any]:
        document: dict[str, Any] = {}
        try:
            document = json.loads(read_text(self.output / result.stdout))
        except json.JSONDecodeError:
            pass
        code = document.get("refusal", {}).get("code")
        safe = (
            result.exit_code == 2
            and document.get("status") == "refused"
            and code == expected_code
            and isinstance(document.get("authority"), dict)
        )
        return {
            "status": "safe_refusal" if safe else "non_actionable_or_unexpected",
            "exit_code": result.exit_code,
            "reported_code": code,
            "expected_code": expected_code,
            "authority_boundary_present": isinstance(
                document.get("authority"), dict
            ),
            "step": result.step_id,
        }

    def primary_config_binary(
        self,
    ) -> tuple[str, Path, list[str]] | None:
        if "nq-suite" in self.installed_binaries:
            return (
                "nq-suite",
                self.installed_binaries["nq-suite"],
                ["config", "validate", "--config"],
            )
        if "nq-monitor" in self.installed_binaries:
            return (
                "nq-monitor",
                self.installed_binaries["nq-monitor"],
                ["config", "validate", "--config"],
            )
        if "nq-witness" in self.installed_binaries:
            return (
                "nq-witness",
                self.installed_binaries["nq-witness"],
                ["config", "validate", "--config"],
            )
        return None

    def classify_failure(
        self, result: StepResult, *, expected: Sequence[str]
    ) -> dict[str, Any]:
        combined = (
            read_text(self.output / result.stdout)
            + "\n"
            + read_text(self.output / result.stderr)
        ).lower()
        present = {fragment: fragment.lower() in combined for fragment in expected}
        return {
            "status": (
                "safe_refusal"
                if result.exit_code not in (None, 0) and all(present.values())
                else "non_actionable_or_unexpected"
            ),
            "exit_code": result.exit_code,
            "timed_out": result.timed_out,
            "expected_message_fragments": present,
            "step": result.step_id,
        }

    def suite_failure_scenarios(self) -> dict[str, Any]:
        binary = self.installed_binaries["nq-suite"]
        base_path = self.config_dir / "suite-failure-base.json"
        source = (
            self.source_root / "crates/nq-suite/examples/minimal-public.json"
            if self.source_root is not None
            else None
        )
        if source is None or not source.is_file():
            return {
                "unknown_pack": {
                    "status": "not_run",
                    "reason": "minimal suite configuration unavailable",
                }
            }
        base = json.loads(source.read_text(encoding="utf-8"))
        write_json(base_path, base)

        unknown_pack = json.loads(json.dumps(base))
        unknown_pack["packs"]["enabled"][0]["pack_id"] = "nq.typo"
        unknown_pack_path = self.config_dir / "unknown-pack.json"
        write_json(unknown_pack_path, unknown_pack)
        pack_result = self.run_step(
            "103-unknown-pack",
            "Exercise an unknown pack ID; no best-effort fallback is allowed",
            [
                str(binary),
                "config",
                "validate",
                "--config",
                str(unknown_pack_path),
            ],
            cwd=self.install_root,
            timeout_s=15,
        )

        unknown_check = json.loads(json.dumps(base))
        unknown_check["packs"]["enabled"][0]["checks"] = ["host.typo"]
        unknown_check_path = self.config_dir / "unknown-check.json"
        write_json(unknown_check_path, unknown_check)
        check_result = self.run_step(
            "104-unknown-check",
            "Exercise an unknown check ID; typo tolerance is forbidden",
            [
                str(binary),
                "config",
                "validate",
                "--config",
                str(unknown_check_path),
            ],
            cwd=self.install_root,
            timeout_s=15,
        )

        unavailable = json.loads(json.dumps(base))
        unavailable["packs"]["enabled"][0] = {
            "pack_id": "nq.labelwatch",
            "checks": ["labelwatch.service_state"],
            "config": {"services": []},
        }
        unavailable_path = self.config_dir / "unavailable-pack.json"
        write_json(unavailable_path, unavailable)
        unavailable_result = self.run_step(
            "105-unavailable-pack",
            (
                "Exercise a known optional pack absent from the default "
                "feature graph; compiling it elsewhere must not enable it"
            ),
            [
                str(binary),
                "config",
                "validate",
                "--config",
                str(unavailable_path),
            ],
            cwd=self.install_root,
            timeout_s=15,
        )
        return {
            "unknown_pack": self.classify_failure(
                pack_result, expected=("unknown pack", "no listener")
            ),
            "unknown_check": self.classify_failure(
                check_result, expected=("unknown check", "no listener")
            ),
            "unavailable_pack": self.classify_failure(
                unavailable_result, expected=("unavailable", "no listener")
            ),
        }

    def occupied_witness_scenario(self) -> dict[str, Any]:
        binary = self.installed_binaries["nq-witness"]
        held = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        held.bind(("127.0.0.1", 0))
        held.listen(1)
        address = held.getsockname()
        config = self.config_dir / "occupied-witness.json"
        write_json(
            config,
            {
                "bind_addr": f"{address[0]}:{address[1]}",
                "sqlite_paths": [],
                "service_health_urls": [],
                "prometheus_targets": [],
                "log_sources": [],
                "sqlite_wal_targets": [],
            },
        )
        try:
            result = self.run_step(
                "106-occupied-witness-port",
                "Exercise an occupied publisher port without running any check",
                [str(binary), "--config", str(config)],
                cwd=self.install_root,
                timeout_s=15,
            )
        finally:
            held.close()
        return self.classify_failure(
            result, expected=("cannot bind publisher listener", "no checks ran")
        )

    def occupied_monitor_scenario(self) -> dict[str, Any]:
        binary = self.installed_binaries["nq-monitor"]
        held = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        held.bind(("127.0.0.1", 0))
        held.listen(1)
        address = held.getsockname()
        database = self.state_dir / "occupied-port-must-not-exist.db"
        config = self.config_dir / "occupied-monitor.json"
        write_json(
            config,
            {
                "interval_s": 10,
                "db_path": str(database),
                "bind_addr": f"{address[0]}:{address[1]}",
                "sources": [],
            },
        )
        try:
            result = self.run_step(
                "107-occupied-monitor-port",
                "Exercise an occupied monitor port before database initialization",
                [str(binary), "serve", "--config", str(config)],
                cwd=self.install_root,
                timeout_s=15,
            )
        finally:
            held.close()
        classified = self.classify_failure(
            result,
            expected=("cannot bind monitor listener", "no database was opened"),
        )
        classified["database_created"] = database.exists()
        if database.exists():
            classified["status"] = "unsafe_state_change"
        return classified

    def unavailable_source_scenario(self) -> dict[str, Any]:
        binary = self.installed_binaries["nq-monitor"]
        source_port = unused_loopback_port()
        monitor_port = unused_loopback_port()
        while monitor_port == source_port:
            monitor_port = unused_loopback_port()
        database = self.state_dir / "unavailable-source.db"
        config = self.config_dir / "unavailable-source.json"
        write_json(
            config,
            {
                "interval_s": 1,
                "db_path": str(database),
                "bind_addr": f"127.0.0.1:{monitor_port}",
                "sources": [
                    {
                        "name": "unavailable-witness",
                        "base_url": f"http://127.0.0.1:{source_port}",
                        "timeout_ms": 250,
                    }
                ],
            },
        )
        process = self.start_process(
            "108-unavailable-source-process",
            "Run the monitor with one explicitly unavailable witness source",
            [str(binary), "serve", "--config", str(config)],
            cwd=self.install_root,
        )
        try:
            if not wait_http(
                f"http://127.0.0.1:{monitor_port}/api/overview", process, 20
            ):
                return {
                    "status": "monitor_failed_to_start",
                    "source_port": source_port,
                    "monitor_port": monitor_port,
                }
            time.sleep(2)
            result = self.run_step(
                "109-unavailable-source-query",
                "Inspect the durable source outcome rather than treating absence as health",
                [
                    str(binary),
                    "query",
                    "--remote",
                    f"http://127.0.0.1:{monitor_port}",
                    (
                        "SELECT source, last_status, last_error "
                        "FROM v_sources ORDER BY source"
                    ),
                ],
                cwd=self.install_root,
                timeout_s=15,
            )
            output = read_text(self.output / result.stdout).lower()
            visible = (
                result.exit_code == 0
                and "unavailable-witness" in output
                and ("error" in output or "failed" in output)
            )
            return {
                "status": (
                    "unavailability_visible" if visible else "unavailability_ambiguous"
                ),
                "exit_code": result.exit_code,
                "source_identity_visible": "unavailable-witness" in output,
                "failure_visible": "error" in output or "failed" in output,
                "underlying_service_changed": False,
            }
        finally:
            self.finish_process("108-unavailable-source-process", process)

    def stale_database_scenario(self) -> dict[str, Any]:
        binary = self.installed_binaries["nq-monitor"]
        database = self.state_dir / "schema-v7.db"
        connection = sqlite3.connect(database)
        connection.execute("CREATE TABLE generations (id INTEGER PRIMARY KEY)")
        connection.execute("PRAGMA user_version = 7")
        connection.commit()
        connection.close()
        before = sha256(database)
        result = self.run_step(
            "110-stale-database-preflight",
            (
                "Inspect a deterministic older NQ schema marker read-only; "
                "the preflight must disclose migration and leave bytes unchanged"
            ),
            [
                str(binary),
                "database",
                "compatibility",
                "--db",
                str(database),
                "--format",
                "json",
            ],
            cwd=self.install_root,
            timeout_s=30,
        )
        after = sha256(database)
        state = None
        try:
            state = json.loads(read_text(self.output / result.stdout)).get("state")
        except json.JSONDecodeError:
            pass
        return {
            "status": (
                "upgrade_disclosed_without_mutation"
                if result.exit_code == 0
                and state == "upgrade_required"
                and before == after
                else "unsafe_or_ambiguous"
            ),
            "reported_state": state,
            "bytes_unchanged": before == after,
            "wal_created": database.with_name(database.name + "-wal").exists(),
            "shm_created": database.with_name(database.name + "-shm").exists(),
        }

    def upgrade_preflight(self) -> None:
        if self.args.prior_database is None:
            self.observations["upgrade"] = {
                "status": "not_run",
                "reason": (
                    "No supported prior release artifact/database is available "
                    "and the harness does not manufacture a passing prior state."
                ),
                "required_input": "--prior-database",
            }
            return
        if "nq-monitor" not in self.installed_binaries:
            self.observations["upgrade"] = {
                "status": "not_applicable",
                "reason": "The selected profile does not install nq-monitor.",
            }
            return
        source = self.args.prior_database.resolve()
        if not source.is_file():
            self.observations["upgrade"] = {
                "status": "blocked",
                "reason": f"prior database does not exist: {source}",
            }
            return
        staged = self.state_dir / "prior-release.db"
        shutil.copy2(source, staged)
        source_before = sha256(source)
        staged_before = sha256(staged)
        binary = self.installed_binaries["nq-monitor"]
        result = self.run_step(
            "120-prior-database-compatibility",
            (
                "Inspect a caller-supplied prior database without migrating it; "
                "the original remains outside the campaign workspace"
            ),
            [
                str(binary),
                "database",
                "compatibility",
                "--db",
                str(staged),
                "--format",
                "json",
            ],
            cwd=self.install_root,
            timeout_s=30,
        )
        report: dict[str, Any] = {}
        try:
            report = json.loads(read_text(self.output / result.stdout))
        except json.JSONDecodeError:
            pass
        self.observations["upgrade"] = {
            "status": "compatibility_inspected",
            "report": report,
            "staged_bytes_unchanged": staged_before == sha256(staged),
            "original_bytes_unchanged": source_before == sha256(source),
            "migration_executed": False,
            "reason_migration_not_executed": (
                "This campaign requires a versioned prior binary/configuration "
                "pair before exercising the mutating startup path."
            ),
        }

    def removal_reset_plan(self) -> None:
        inventory = []
        for path in sorted(self.install_root.rglob("*")):
            if not path.is_file():
                continue
            relative = path.relative_to(self.install_root)
            name = path.name
            if name.endswith((".db", ".db-wal", ".db-shm")):
                classification = "durable_evidence"
                rule = "archive database and sidecars as one stopped-writer set"
            elif "config" in relative.parts:
                classification = "durable_operator_record"
                rule = "archive to preserve the observation and deployment basis"
            elif name == "liveness.json":
                classification = "replaceable_derived_state"
                rule = "may be recreated after durable state is preserved"
            elif "bin" in relative.parts:
                classification = "replaceable_artifact"
                rule = "record version and digest; removal does not reset evidence"
            else:
                classification = "campaign_fixture_or_runtime_output"
                rule = "inspect before removal"
            inventory.append(
                {
                    "path": str(relative),
                    "bytes": path.stat().st_size,
                    "sha256": sha256(path),
                    "classification": classification,
                    "removal_rule": rule,
                }
            )
        plan = {
            "schema": "nq.install_first_run.removal_plan.v1",
            "generated_at": utc_now(),
            "action_executed": False,
            "archive_first": True,
            "items": inventory,
            "safe_sequence": [
                "stop writers",
                "record binary versions and configuration paths",
                "archive configuration and the database plus matching sidecars",
                "verify the archive independently",
                "move the live set to a dated quarantine",
                "start a new database only when loss of prior operational history is intentional",
            ],
            "refusal": (
                "The campaign does not delete or reset durable evidence merely "
                "to make installation or first use pass."
            ),
        }
        write_json(self.output / "removal-reset-plan.json", plan)
        self.observations["removal_and_reset"] = {
            "status": "classified_without_deletion",
            "inventory_count": len(inventory),
            "durable_evidence_count": sum(
                item["classification"] == "durable_evidence" for item in inventory
            ),
            "plan": "removal-reset-plan.json",
        }

    def elapsed_ms(self) -> int:
        return (time.monotonic_ns() - self.started_ns) // 1_000_000

    def finish(self) -> int:
        self.removal_reset_plan()
        duration_ms = self.elapsed_ms()
        status = (
            "blocked"
            if self.blocker is not None
            else "profile_first_use_completed"
            if self.profile_result_at_ms is not None
            else "installed_without_first_use"
        )
        verdicts = {
            "self_contained": (
                self.blocker is None
                and self.args.track == "release"
                and self.profile_result_at_ms is not None
            ),
            "composable": (
                self.args.profile == "suite-minimal"
                and self.profile_result_at_ms is not None
                and self.observations.get("first_use", {}).get("conservative_host_only")
                is True
            ),
            "recoverable": self.recovery_verdict(),
            "suitable_for_non_author": (
                self.host_result_at_ms is not None
                and self.recovery_verdict()
                and self.observations.get("environment_leaks", {}).get("status")
                == "not_detected"
            ),
        }
        manifest = {
            "schema": CAMPAIGN_SCHEMA,
            "status": status,
            "track": self.args.track,
            "profile": self.args.profile,
            "dependency_mode": self.args.dependency_mode,
            "started_at": self.started_at,
            "finished_at": utc_now(),
            "duration_ms": duration_ms,
            "time_to_first_profile_result_ms": self.profile_result_at_ms,
            "time_to_first_meaningful_host_result_ms": self.host_result_at_ms,
            "blocker": self.blocker,
            "observations": self.observations,
            "steps": [asdict(step) for step in self.steps],
            "workspace_retained": self.args.keep_workspace,
            "workspace": str(self.workspace) if self.args.keep_workspace else None,
            "raw_evidence_policy": (
                "Step stdout and stderr are unedited. Curated conclusions are "
                "stored separately in this manifest and the failure matrix."
            ),
            "verdicts": verdicts,
        }
        write_json(self.output / "manifest.json", manifest)
        (self.output / "workspace-tree.tsv").write_text(
            "\n".join(bounded_workspace_inventory(self.workspace)) + "\n",
            encoding="utf-8",
        )
        if not self.args.keep_workspace:
            shutil.rmtree(self.workspace)
        return 2 if self.blocker else 0

    def recovery_verdict(self) -> bool:
        matrix = self.observations.get("failure_and_recovery")
        if not isinstance(matrix, dict):
            return False
        required = ("wrong_path", "malformed_configuration")
        return all(
            isinstance(matrix.get(name), dict)
            and matrix[name].get("status") == "safe_refusal"
            for name in required
        )

    def run(self) -> int:
        self.record_environment()
        self.exercise_missing_dependency()
        installed = False
        if self.args.track == "source-archive":
            if self.prepare_source_archive():
                assert self.source_root is not None
                path = (
                    self.args.profiles
                    if self.args.profiles is not None
                    else self.source_root
                    / "docs"
                    / "install"
                    / "INSTALLATION_PROFILES.json"
                )
                basis = (
                    "explicit_evaluator_override"
                    if self.args.profiles is not None
                    else "inside_committed_source_archive"
                )
                if self.load_profiles(path, basis):
                    installed = self.install_source_profile()
        else:
            path = (
                self.args.profiles
                if self.args.profiles is not None
                else default_profiles_path()
            )
            basis = (
                "explicit_evaluator_override"
                if self.args.profiles is not None
                else "evaluator_contract_not_packaged_in_release"
            )
            if self.load_profiles(path, basis):
                installed = self.install_release_profile()
        self.observations["installation"] = {
            "status": "installed" if installed else "blocked",
            "installed_binaries": sorted(self.installed_binaries),
            "prefix": str(self.install_root),
            "system_permission_required": False,
            "prompt_response_supplied": False,
        }
        if installed:
            self.first_use()
            self.failure_matrix()
            self.upgrade_preflight()
        else:
            self.observations.setdefault(
                "upgrade",
                {
                    "status": "not_run",
                    "reason": "Installation did not produce nq-monitor.",
                },
            )
        return self.finish()


def inspect_source_archive(path: Path) -> dict[str, Any]:
    if not tarfile.is_tarfile(path):
        raise ValueError("source input is not a readable tar archive")
    member_count = 0
    byte_count = 0
    top_levels: set[str] = set()
    commit_id: str | None = None
    with tarfile.open(path, "r:*") as archive:
        comment = archive.pax_headers.get("comment")
        if isinstance(comment, str) and len(comment) == 40:
            try:
                int(comment, 16)
            except ValueError:
                pass
            else:
                commit_id = comment
        for member in archive.getmembers():
            member_count += 1
            byte_count += member.size
            pure = PurePosixPath(member.name)
            if pure.is_absolute() or ".." in pure.parts or not pure.parts:
                raise ValueError(f"unsafe archive member path: {member.name!r}")
            if member.issym() or member.islnk():
                raise ValueError(f"archive links are not accepted: {member.name!r}")
            if member.isdev() or member.isfifo():
                raise ValueError(f"archive special files are not accepted: {member.name!r}")
            if ".git" in pure.parts:
                raise ValueError(
                    f"source archive contains checkout metadata: {member.name!r}"
                )
            top_levels.add(pure.parts[0])
    if len(top_levels) != 1:
        raise ValueError(
            f"source archive must have one top-level directory, found {sorted(top_levels)}"
        )
    return {
        "member_count": member_count,
        "uncompressed_bytes": byte_count,
        "top_level": next(iter(top_levels)),
        "git_archive_commit": commit_id,
        "committed_source_basis": commit_id is not None,
        "origin_limit": (
            "A Git archive commit header binds the tree to a commit."
            if commit_id is not None
            else (
                "No Git archive commit header was present. The archive digest "
                "identifies bytes but does not by itself prove which commit produced them."
            )
        ),
    }


def malformed_config_safety_fragments(binary_name: str) -> tuple[str, ...]:
    """Return the component's explicit fail-before-side-effects wording."""

    expected = {
        "nq-suite": ("unknown field", "no listener"),
        "nq-monitor": (
            "unknown field",
            "no database was opened",
            "no listener was started",
        ),
        "nq-witness": (
            "unknown field",
            "no listener was started",
            "no checks ran",
        ),
    }
    try:
        return expected[binary_name]
    except KeyError as error:
        raise ValueError(
            f"no malformed-configuration safety contract for {binary_name!r}"
        ) from error


def unused_loopback_port() -> int:
    held = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        held.bind(("127.0.0.1", 0))
        return int(held.getsockname()[1])
    finally:
        held.close()


def wait_http(
    url: str, process: subprocess.Popen[str], timeout_seconds: float
) -> bool:
    deadline = time.monotonic() + timeout_seconds
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    while time.monotonic() < deadline:
        if process.poll() is not None:
            return False
        try:
            with opener.open(url, timeout=1.0) as response:
                if 200 <= response.status < 300:
                    response.read()
                    return True
        except (urllib.error.URLError, TimeoutError, ConnectionError):
            pass
        time.sleep(0.5)
    return False


def terminate(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=5)
    except (ProcessLookupError, subprocess.TimeoutExpired):
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)


def bounded_workspace_inventory(workspace: Path) -> list[str]:
    opaque = {".cargo", ".rustup", "target"}
    lines: list[str] = []
    for path in sorted(workspace.rglob("*")):
        relative = path.relative_to(workspace)
        if any(part in opaque for part in relative.parts):
            if path.is_dir() and path.name in opaque:
                files = [child for child in path.rglob("*") if child.is_file()]
                lines.append(
                    "opaque-dir\t"
                    f"{sum(child.stat().st_size for child in files)}\t"
                    f"{relative}\tfiles={len(files)}"
                )
            continue
        if len(relative.parts) > 5:
            continue
        kind = "d" if path.is_dir() else "f"
        size = "-" if path.is_dir() else str(path.stat().st_size)
        lines.append(f"{kind}\t{size}\t{relative}")
    return lines


def default_profiles_path() -> Path:
    return (
        Path(__file__).resolve().parent.parent
        / "docs"
        / "install"
        / "INSTALLATION_PROFILES.json"
    )


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run one NQ install/first-use profile in an isolated environment. "
            "Exit 2 is a preserved product-path block, not a harness crash."
        )
    )
    parser.add_argument(
        "--track",
        choices=("source-archive", "release"),
        required=True,
    )
    parser.add_argument(
        "--profile",
        choices=(
            "suite-minimal",
            "legacy-operational",
            "monitor-dashboard-only",
            "witness-artifact",
        ),
        required=True,
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-archive", type=Path)
    parser.add_argument("--prior-database", type=Path)
    parser.add_argument(
        "--profiles",
        type=Path,
        help=(
            "Explicit evaluator override. Source-archive runs otherwise load "
            "the profile contract from inside the archive."
        ),
    )
    parser.add_argument("--release-base", default=DEFAULT_RELEASE_BASE)
    parser.add_argument(
        "--dependency-mode",
        choices=("isolated-offline", "isolated-online"),
        default="isolated-offline",
        help=(
            "Offline forbids Cargo/Rustup downloads and exposes missing "
            "packaging as evidence. Online still inherits no proxy or credential state."
        ),
    )
    parser.add_argument("--build-timeout", type=float, default=1800)
    parser.add_argument("--download-timeout", type=float, default=120)
    parser.add_argument("--keep-workspace", action="store_true")
    return parser.parse_args(list(argv))


def main(argv: Iterable[str]) -> int:
    arguments = parse_args(argv)
    try:
        campaign = Campaign(arguments)
        return campaign.run()
    except (OSError, ValueError, RuntimeError) as error:
        print(f"install campaign instrumentation failed: {error}", file=sys.stderr)
        return 3


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
