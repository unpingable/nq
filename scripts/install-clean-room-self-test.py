#!/usr/bin/env python3
"""Deterministic fail-path test for install-clean-room.py."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


def main() -> int:
    repo = Path(__file__).resolve().parent.parent
    harness = repo / "scripts" / "install-clean-room.py"
    with tempfile.TemporaryDirectory(
        prefix="nq-install-harness-test-", dir="/tmp"
    ) as tmp:
        output = Path(tmp) / "evidence"
        missing = Path(tmp) / "does-not-exist.git"
        completed = subprocess.run(
            [
                sys.executable,
                str(harness),
                "--track",
                "source",
                "--output",
                str(output),
                "--repository",
                missing.as_uri(),
            ],
            cwd="/tmp",
            env={
                "PATH": "/usr/local/bin:/usr/bin:/bin",
                "LANG": "C",
                "LC_ALL": "C",
            },
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if completed.returncode != 2:
            print(
                f"expected blocked-path exit 2, got {completed.returncode}: "
                f"{completed.stderr}",
                file=sys.stderr,
            )
            return 1
        manifest = json.loads((output / "manifest.json").read_text(encoding="utf-8"))
        environment = json.loads(
            (output / "environment.json").read_text(encoding="utf-8")
        )
        assert manifest["schema"] == "nq.install_clean_room.v1"
        assert manifest["status"] == "blocked"
        assert manifest["failure_step"] == "010-source-install"
        assert manifest["time_to_first_meaningful_result_ms"] is None
        assert environment["inherited_environment_variable_count"] == 0
        effective = environment["effective_environment"]
        assert effective["HOME"].startswith("/tmp/nq-install-source-")
        assert effective["CARGO_HOME"].startswith(effective["HOME"])
        assert effective["RUSTUP_HOME"].startswith(effective["HOME"])
        forbidden = {
            "NQ_CONFIG",
            "NQ_DB",
            "RUSTFLAGS",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "SSH_AUTH_SOCK",
        }
        assert forbidden.isdisjoint(effective)
        assert not list(Path(tmp).glob("**/nq-witness"))
        assert not list(Path(tmp).glob("**/nq-monitor"))
    print("install clean-room harness self-test: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
