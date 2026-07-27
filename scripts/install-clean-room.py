#!/usr/bin/env python3
"""Run NQ's documented install path without inheriting a developer checkout.

This is campaign instrumentation, not an installer.  A failed install is a
successful observation: the harness records the failure and does not install a
missing prerequisite, reuse a developer cache, or silently choose another
path.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Sequence


SCHEMA = "nq.install_clean_room.v1"
DEFAULT_REPOSITORY = "https://github.com/unpingable/nq.git"
DEFAULT_RELEASE_BASE = "https://github.com/unpingable/nq/releases/latest/download"
DEFAULT_PATH = "/usr/local/bin:/usr/bin:/bin"

PUBLISHER_CONFIG = """\
{
  "bind_addr": "127.0.0.1:9847",
  "sqlite_paths": [],
  "service_health_urls": [],
  "prometheus_targets": [],
  "log_sources": [],
  "sqlite_wal_targets": []
}
"""

AGGREGATOR_CONFIG = """\
{
  "interval_s": 10,
  "db_path": "./nq.db",
  "bind_addr": "127.0.0.1:9848",
  "sources": [
    {
      "name": "local-host",
      "base_url": "http://127.0.0.1:9847",
      "timeout_ms": 5000
    }
  ],
  "retention": {
    "max_generations": 360,
    "prune_every_n_cycles": 60
  },
  "notifications": {
    "channels": [],
    "min_severity": "warning"
  },
  "liveness": {
    "path": "./liveness.json",
    "instance_id": "quickstart"
  }
}
"""


@dataclass(frozen=True)
class StepResult:
    step_id: str
    description: str
    command: list[str]
    cwd: str
    started_at: str
    finished_at: str
    duration_ms: int
    exit_code: int
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


class CleanRoom:
    def __init__(
        self,
        *,
        track: str,
        output: Path,
        keep_workspace: bool,
        repository: str,
        release_base: str,
        first_run_policy: str,
    ) -> None:
        self.track = track
        self.output = output.resolve()
        self.keep_workspace = keep_workspace
        self.repository = repository
        self.release_base = release_base.rstrip("/")
        self.first_run_policy = first_run_policy
        self.started_at = utc_now()
        self.started_ns = time.monotonic_ns()
        self.steps: list[StepResult] = []
        self.failure_step: str | None = None
        self.first_meaningful_result = False
        self.first_meaningful_result_at: str | None = None
        self.first_meaningful_result_ms: int | None = None
        if self.output.exists():
            raise ValueError(f"output already exists: {self.output}")
        self.workspace = Path(
            tempfile.mkdtemp(prefix=f"nq-install-{track}-", dir="/tmp")
        )
        self.home = self.workspace / "home"
        self.work = self.workspace / "work"
        self.home.mkdir(mode=0o700)
        self.work.mkdir()
        self.clean_env = {
            "HOME": str(self.home),
            "XDG_CONFIG_HOME": str(self.home / ".config"),
            "XDG_CACHE_HOME": str(self.home / ".cache"),
            "CARGO_HOME": str(self.home / ".cargo"),
            "RUSTUP_HOME": str(self.home / ".rustup"),
            "PATH": DEFAULT_PATH,
            "LANG": "C",
            "LC_ALL": "C",
            "TZ": "UTC",
            "GIT_TERMINAL_PROMPT": "0",
        }
        (self.output / "steps").mkdir(parents=True)

    def record_environment(self) -> None:
        commands = {}
        for command in (
            "bash",
            "curl",
            "git",
            "cargo",
            "rustc",
            "sha256sum",
            "install",
        ):
            commands[command] = shutil.which(command, path=DEFAULT_PATH)
        value = {
            "schema": SCHEMA,
            "captured_at": utc_now(),
            "platform": {
                "system": platform.system(),
                "release": platform.release(),
                "machine": platform.machine(),
            },
            "effective_environment": self.clean_env,
            "inherited_environment_variable_count": 0,
            "visible_commands": commands,
            "workspace": str(self.workspace),
            "sibling_checkout_search_roots": [str(self.workspace)],
            "note": (
                "Only the variables above are passed to product-path commands. "
                "No inherited NQ, Cargo, Rustup, proxy, credential, or checkout "
                "environment is available."
            ),
        }
        write_json(self.output / "environment.json", value)

    def run_step(
        self,
        step_id: str,
        description: str,
        command: Sequence[str],
        *,
        cwd: Path,
        stop_on_failure: bool = True,
        env_additions: dict[str, str] | None = None,
    ) -> StepResult:
        step_dir = self.output / "steps" / step_id
        step_dir.mkdir()
        environment = dict(self.clean_env)
        if env_additions:
            environment.update(env_additions)
        command_list = [str(part) for part in command]
        write_json(
            step_dir / "invocation.json",
            {
                "description": description,
                "argv": command_list,
                "cwd": str(cwd),
                "environment_additions": env_additions or {},
            },
        )
        start_at = utc_now()
        start_ns = time.monotonic_ns()
        completed = subprocess.run(
            command_list,
            cwd=cwd,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        finish_ns = time.monotonic_ns()
        finish_at = utc_now()
        stdout_path = step_dir / "stdout.log"
        stderr_path = step_dir / "stderr.log"
        stdout_path.write_text(completed.stdout, encoding="utf-8")
        stderr_path.write_text(completed.stderr, encoding="utf-8")
        result = StepResult(
            step_id=step_id,
            description=description,
            command=command_list,
            cwd=str(cwd),
            started_at=start_at,
            finished_at=finish_at,
            duration_ms=(finish_ns - start_ns) // 1_000_000,
            exit_code=completed.returncode,
            stdout=str(stdout_path.relative_to(self.output)),
            stderr=str(stderr_path.relative_to(self.output)),
        )
        write_json(step_dir / "result.json", asdict(result))
        self.steps.append(result)
        if completed.returncode != 0 and stop_on_failure:
            self.failure_step = step_id
        return result

    def inventory(self) -> None:
        self.run_step(
            "000-command-inventory",
            (
                "Record documented command paths without invoking Cargo or "
                "Rustup before the source-install step"
            ),
            [
                "/usr/bin/bash",
                "--noprofile",
                "--norc",
                "-c",
                (
                    "for command in bash curl git cargo rustc sha256sum install; do "
                    'printf "%s\\t" "$command"; '
                    'if command -v "$command" >/dev/null 2>&1; then '
                    'command -v "$command"; '
                    'else echo "MISSING"; fi; '
                    "done"
                ),
            ],
            cwd=self.work,
            stop_on_failure=False,
        )

    def release_install(self) -> Path | None:
        quickstart = self.work / "nq-quickstart"
        script = f"""\
set -eu
mkdir nq-quickstart || exit 1
cd nq-quickstart || exit 1

case "$(uname -m)" in
  x86_64)         arch=amd64 ;;
  aarch64|arm64)  arch=arm64 ;;
  *) echo "No NQ release binary for $(uname -m)" >&2; exit 1 ;;
esac

stage="$(mktemp -d .nq-download.XXXXXX)"
trap 'rm -rf "$stage"' EXIT
base={shell_quote(self.release_base)}
for bin in nq-monitor nq-witness; do
  curl -fL "$base/$bin-linux-$arch" -o "$stage/$bin-linux-$arch"
  curl -fL "$base/$bin-linux-$arch.sha256" \\
    -o "$stage/$bin-linux-$arch.sha256"
  (cd "$stage" && sha256sum --check "$bin-linux-$arch.sha256")
done
for bin in nq-monitor nq-witness; do
  install -m 0755 "$stage/$bin-linux-$arch" "$bin"
done
"""
        result = self.run_step(
            "010-release-install",
            (
                "Literal release-artifact installation block from "
                "docs/operator/quickstart.md"
            ),
            ["/usr/bin/bash", "--noprofile", "--norc", "-x", "-c", script],
            cwd=self.work,
        )
        return quickstart if result.exit_code == 0 else None

    def source_install(self) -> Path | None:
        source = self.work / "nq"
        script = f"""\
set -eu
git clone {shell_quote(self.repository)} || exit 1
cd nq || exit 1
(
  set -eu
  cargo build --release --locked
  install -m 0755 target/release/nq-monitor ./nq-monitor
  install -m 0755 target/release/nq-witness ./nq-witness
)
"""
        result = self.run_step(
            "010-source-install",
            (
                "Literal source installation block from "
                "docs/operator/quickstart.md, with the documented destination "
                "named nq"
            ),
            ["/usr/bin/bash", "--noprofile", "--norc", "-x", "-c", script],
            cwd=self.work,
        )
        if result.exit_code != 0:
            return None
        self.run_step(
            "011-installed-source-identity",
            "Record the exact public source revision and remote after installation",
            [
                "/usr/bin/bash",
                "--noprofile",
                "--norc",
                "-c",
                ("git rev-parse HEAD; git remote get-url origin; git status --short"),
            ],
            cwd=source,
            stop_on_failure=False,
        )
        self.run_step(
            "012-installed-binary-identity",
            "Record installed binary names and reported versions",
            [
                "/usr/bin/bash",
                "--noprofile",
                "--norc",
                "-c",
                "./nq-monitor --version; ./nq-witness --version",
            ],
            cwd=source,
            stop_on_failure=False,
        )
        return source

    def materialize_configs(self, install_dir: Path) -> bool:
        if self.first_run_policy == "literal":
            marker = self.output / "steps" / "020-config-materialization-blocked"
            marker.mkdir()
            write_json(
                marker / "result.json",
                {
                    "step_id": "020-config-materialization-blocked",
                    "description": (
                        "The quickstart says “Save this as” but supplies no "
                        "literal file-creation command. Literal policy refuses "
                        "to infer an editor or shell redirection."
                    ),
                    "exit_code": 78,
                    "duration_ms": 0,
                    "operator_assistance": False,
                },
            )
            self.failure_step = "020-config-materialization-blocked"
            return False
        start = time.monotonic_ns()
        publisher = install_dir / "publisher.json"
        aggregator = install_dir / "aggregator.json"
        publisher.write_text(PUBLISHER_CONFIG, encoding="utf-8")
        aggregator.write_text(AGGREGATOR_CONFIG, encoding="utf-8")
        step_dir = self.output / "steps" / "020-copy-visible-config"
        step_dir.mkdir()
        write_json(
            step_dir / "result.json",
            {
                "step_id": "020-copy-visible-config",
                "description": (
                    "Evaluator materialized the two JSON specimens exactly as "
                    "shown in docs/operator/quickstart.md. This is recorded "
                    "assistance because the document supplies content but no "
                    "file-creation command."
                ),
                "exit_code": 0,
                "duration_ms": (time.monotonic_ns() - start) // 1_000_000,
                "operator_assistance": True,
                "files": {
                    "publisher.json": {
                        "sha256": sha256(publisher),
                        "bytes": publisher.stat().st_size,
                    },
                    "aggregator.json": {
                        "sha256": sha256(aggregator),
                        "bytes": aggregator.stat().st_size,
                    },
                },
            },
        )
        return True

    def validate_configs(self, install_dir: Path) -> bool:
        installed_quickstart = install_dir / "docs" / "operator" / "quickstart.md"
        if installed_quickstart.is_file():
            documented = installed_quickstart.read_text(encoding="utf-8")
            if (
                "./nq-witness config validate --config publisher.json" not in documented
                or "./nq-monitor config validate --config aggregator.json"
                not in documented
            ):
                marker = self.output / "steps" / "030-config-validation-not-documented"
                marker.mkdir()
                write_json(
                    marker / "result.json",
                    {
                        "step_id": "030-config-validation-not-documented",
                        "description": (
                            "The installed source checkout's quickstart does "
                            "not document config validation, so the clean-room "
                            "operator did not invent that step."
                        ),
                        "documentation": str(
                            installed_quickstart.relative_to(install_dir)
                        ),
                        "exit_code": 0,
                        "duration_ms": 0,
                        "operator_assistance": False,
                    },
                )
                return True
        witness = self.run_step(
            "030-validate-witness-config",
            "Run the documented side-effect-free witness config validation",
            [
                str(install_dir / "nq-witness"),
                "config",
                "validate",
                "--config",
                "publisher.json",
            ],
            cwd=install_dir,
        )
        if witness.exit_code != 0:
            return False
        monitor = self.run_step(
            "031-validate-monitor-config",
            "Run the documented side-effect-free monitor config validation",
            [
                str(install_dir / "nq-monitor"),
                "config",
                "validate",
                "--config",
                "aggregator.json",
            ],
            cwd=install_dir,
        )
        return monitor.exit_code == 0

    def first_run(self, install_dir: Path) -> None:
        if not self.materialize_configs(install_dir):
            return
        if not self.validate_configs(install_dir):
            return

        witness_dir = self.output / "steps" / "040-witness-process"
        monitor_dir = self.output / "steps" / "050-monitor-process"
        witness_dir.mkdir()
        monitor_dir.mkdir()
        witness_out = (witness_dir / "stdout.log").open("w", encoding="utf-8")
        witness_err = (witness_dir / "stderr.log").open("w", encoding="utf-8")
        monitor_out = (monitor_dir / "stdout.log").open("w", encoding="utf-8")
        monitor_err = (monitor_dir / "stderr.log").open("w", encoding="utf-8")
        witness: subprocess.Popen[str] | None = None
        monitor: subprocess.Popen[str] | None = None
        witness_was_running = False
        monitor_was_running = False
        witness_start = utc_now()
        witness_ns = time.monotonic_ns()
        monitor_start: str | None = None
        monitor_ns: int | None = None
        try:
            witness = subprocess.Popen(
                [str(install_dir / "nq-witness"), "--config", "publisher.json"],
                cwd=install_dir,
                env=self.clean_env,
                text=True,
                stdout=witness_out,
                stderr=witness_err,
                start_new_session=True,
            )
            time.sleep(0.1)
            if not wait_http("http://127.0.0.1:9847/state", witness, 30.0):
                self.failure_step = "040-witness-process"
                return
            witness_state = self.run_step(
                "041-witness-state",
                "Run the documented curl request against witness /state",
                ["/usr/bin/curl", "-fsS", "http://127.0.0.1:9847/state"],
                cwd=install_dir,
            )
            if witness_state.exit_code != 0:
                return

            monitor_start = utc_now()
            monitor_ns = time.monotonic_ns()
            monitor = subprocess.Popen(
                [
                    str(install_dir / "nq-monitor"),
                    "serve",
                    "--config",
                    "aggregator.json",
                ],
                cwd=install_dir,
                env=self.clean_env,
                text=True,
                stdout=monitor_out,
                stderr=monitor_err,
                start_new_session=True,
            )
            if not wait_http("http://127.0.0.1:9848/api/overview", monitor, 30.0):
                self.failure_step = "050-monitor-process"
                return
            overview = self.run_step(
                "051-overview",
                "Run the documented curl request against the overview API",
                ["/usr/bin/curl", "-fsS", "http://127.0.0.1:9848/api/overview"],
                cwd=install_dir,
            )
            if overview.exit_code != 0:
                return

            result = self.run_step(
                "060-v-hosts-initial",
                (
                    "Run the documented v_hosts query after HTTP readiness; "
                    "readiness may precede the first generation"
                ),
                [
                    str(install_dir / "nq-monitor"),
                    "query",
                    "--remote",
                    "http://127.0.0.1:9848",
                    (
                        "SELECT host, cpu_load_1m, mem_pressure_pct, "
                        "disk_used_pct, age_s FROM v_hosts"
                    ),
                ],
                cwd=install_dir,
            )
            output = (self.output / result.stdout).read_text(encoding="utf-8")
            if result.exit_code == 0 and "local-host" not in output:
                wait = self.run_step(
                    "061-documented-observation-wait",
                    "Wait the one additional 10-second interval allowed by the quickstart",
                    ["/usr/bin/sleep", "10"],
                    cwd=install_dir,
                )
                if wait.exit_code != 0:
                    return
                result = self.run_step(
                    "062-v-hosts-after-wait",
                    "Repeat the documented v_hosts query after one interval",
                    [
                        str(install_dir / "nq-monitor"),
                        "query",
                        "--remote",
                        "http://127.0.0.1:9848",
                        (
                            "SELECT host, cpu_load_1m, mem_pressure_pct, "
                            "disk_used_pct, age_s FROM v_hosts"
                        ),
                    ],
                    cwd=install_dir,
                )
                output = (self.output / result.stdout).read_text(encoding="utf-8")
            if result.exit_code == 0 and "local-host" in output:
                self.first_meaningful_result = True
                self.first_meaningful_result_at = utc_now()
                self.first_meaningful_result_ms = (
                    time.monotonic_ns() - self.started_ns
                ) // 1_000_000
            else:
                self.failure_step = result.step_id
        finally:
            monitor_was_running = monitor is not None and monitor.poll() is None
            witness_was_running = witness is not None and witness.poll() is None
            terminate(monitor)
            terminate(witness)
            witness_out.close()
            witness_err.close()
            monitor_out.close()
            monitor_err.close()
            witness_finished = utc_now()
            write_json(
                witness_dir / "result.json",
                {
                    "step_id": "040-witness-process",
                    "started_at": witness_start,
                    "finished_at": witness_finished,
                    "duration_ms": (time.monotonic_ns() - witness_ns) // 1_000_000,
                    "exit_code": None if witness is None else witness.poll(),
                    "terminated_by_harness": witness_was_running,
                },
            )
            write_json(
                monitor_dir / "result.json",
                {
                    "step_id": "050-monitor-process",
                    "started_at": monitor_start,
                    "finished_at": utc_now(),
                    "duration_ms": (
                        None
                        if monitor_ns is None
                        else (time.monotonic_ns() - monitor_ns) // 1_000_000
                    ),
                    "exit_code": None if monitor is None else monitor.poll(),
                    "terminated_by_harness": monitor_was_running,
                },
            )

    def finish(self) -> int:
        finished_at = utc_now()
        duration_ms = (time.monotonic_ns() - self.started_ns) // 1_000_000
        status = (
            "first_meaningful_result"
            if self.first_meaningful_result
            else "blocked"
            if self.failure_step
            else "installed_without_meaningful_result"
        )
        tree_lines = bounded_workspace_inventory(self.workspace)
        (self.output / "workspace-tree.tsv").write_text(
            "\n".join(tree_lines) + ("\n" if tree_lines else ""),
            encoding="utf-8",
        )
        write_json(
            self.output / "manifest.json",
            {
                "schema": SCHEMA,
                "track": self.track,
                "status": status,
                "started_at": self.started_at,
                "finished_at": finished_at,
                "duration_ms": duration_ms,
                "time_to_first_meaningful_result_ms": self.first_meaningful_result_ms,
                "first_meaningful_result_at": self.first_meaningful_result_at,
                "failure_step": self.failure_step,
                "repository": self.repository if self.track == "source" else None,
                "release_base": (
                    self.release_base if self.track == "release" else None
                ),
                "first_run_policy": self.first_run_policy,
                "workspace_retained": self.keep_workspace,
                "workspace": str(self.workspace) if self.keep_workspace else None,
                "steps": [asdict(step) for step in self.steps],
                "interpretation": (
                    "A blocked status is installation evidence, not a harness "
                    "failure. The harness did not repair the environment."
                ),
            },
        )
        if not self.keep_workspace:
            shutil.rmtree(self.workspace)
        return 0 if self.first_meaningful_result else 2

    def run(self) -> int:
        self.record_environment()
        self.inventory()
        install_dir = (
            self.release_install() if self.track == "release" else self.source_install()
        )
        if install_dir is not None:
            self.first_run(install_dir)
        return self.finish()


def shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\"'\"'") + "'"


def bounded_workspace_inventory(workspace: Path) -> list[str]:
    """Inventory outcomes without committing millions of build-tree paths."""

    opaque_directories = {".cargo", ".git", ".rustup", "target"}
    lines: list[str] = []
    for path in sorted(workspace.rglob("*")):
        relative = path.relative_to(workspace)
        if any(part in opaque_directories for part in relative.parts):
            if path.is_dir() and path.name in opaque_directories:
                file_count = 0
                byte_count = 0
                for child in path.rglob("*"):
                    if child.is_file():
                        file_count += 1
                        byte_count += child.stat().st_size
                lines.append(
                    f"opaque-dir\t{byte_count}\t{relative}\tfiles={file_count}"
                )
            continue
        if len(relative.parts) > 4:
            continue
        kind = "d" if path.is_dir() else "f"
        size = "-" if path.is_dir() else str(path.stat().st_size)
        lines.append(f"{kind}\t{size}\t{relative}")
    return lines


def wait_http(url: str, process: subprocess.Popen[str], timeout_seconds: float) -> bool:
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
        time.sleep(1.0)
    return False


def terminate(process: subprocess.Popen[str] | None) -> None:
    if process is None or process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=5)
    except (ProcessLookupError, subprocess.TimeoutExpired):
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Exercise one documented NQ install path in an empty /tmp HOME and "
            "write raw, machine-readable evidence. Exit 2 means the product "
            "path was blocked; it is still a completed campaign observation."
        )
    )
    parser.add_argument("--track", choices=("release", "source"), required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--repository",
        default=DEFAULT_REPOSITORY,
        help="source URL; override is recorded and intended only for harness tests",
    )
    parser.add_argument(
        "--release-base",
        default=DEFAULT_RELEASE_BASE,
        help="release asset base URL; override is recorded",
    )
    parser.add_argument(
        "--first-run-policy",
        choices=("literal", "copy-visible-config"),
        default="copy-visible-config",
        help=(
            "literal stops at the undocumented “Save this as” operation; "
            "copy-visible-config records evaluator materialization and proceeds"
        ),
    )
    parser.add_argument(
        "--keep-workspace",
        action="store_true",
        help="retain the isolated /tmp workspace and record its path",
    )
    return parser.parse_args(list(argv))


def main(argv: Iterable[str] = sys.argv[1:]) -> int:
    args = parse_args(argv)
    try:
        campaign = CleanRoom(
            track=args.track,
            output=args.output,
            keep_workspace=args.keep_workspace,
            repository=args.repository,
            release_base=args.release_base,
            first_run_policy=args.first_run_policy,
        )
        return campaign.run()
    except (OSError, ValueError) as error:
        print(f"install clean-room harness error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
