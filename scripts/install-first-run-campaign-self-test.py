#!/usr/bin/env python3
"""Offline, deterministic tests for install-first-run-campaign.py."""

from __future__ import annotations

import importlib.util
import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path


def load_harness(path: Path):
    spec = importlib.util.spec_from_file_location("nq_install_campaign", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load campaign harness")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def add_bytes(archive: tarfile.TarFile, name: str, payload: bytes) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(payload)
    info.mode = 0o644
    archive.addfile(info, io.BytesIO(payload))


def main() -> int:
    repo = Path(__file__).resolve().parent.parent
    harness_path = repo / "scripts" / "install-first-run-campaign.py"
    harness = load_harness(harness_path)
    with tempfile.TemporaryDirectory(
        prefix="nq-first-run-harness-test-", dir="/tmp"
    ) as directory:
        root = Path(directory)

        ordinary_archive = root / "ordinary.tar"
        with tarfile.open(ordinary_archive, "w") as archive:
            add_bytes(archive, "nq/Cargo.toml", b"[workspace]\n")
        inspected = harness.inspect_source_archive(ordinary_archive)
        assert inspected["top_level"] == "nq"
        assert inspected["committed_source_basis"] is False

        unsafe_archive = root / "unsafe.tar"
        with tarfile.open(unsafe_archive, "w") as archive:
            link = tarfile.TarInfo("nq/escape")
            link.type = tarfile.SYMTYPE
            link.linkname = "../../outside"
            archive.addfile(link)
        try:
            harness.inspect_source_archive(unsafe_archive)
        except ValueError as error:
            assert "links are not accepted" in str(error)
        else:
            raise AssertionError("unsafe source archive was accepted")

        output = root / "evidence"
        missing = root / "missing.tar"
        completed = subprocess.run(
            [
                sys.executable,
                str(harness_path),
                "--track",
                "source-archive",
                "--profile",
                "suite-minimal",
                "--source-archive",
                str(missing),
                "--output",
                str(output),
            ],
            cwd="/tmp",
            env={
                "PATH": "/usr/local/bin:/usr/bin:/bin",
                "LANG": "C",
                "LC_ALL": "C",
                "PYTHONDONTWRITEBYTECODE": "1",
            },
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=30,
        )
        assert completed.returncode == 2, completed.stderr
        manifest = json.loads((output / "manifest.json").read_text(encoding="utf-8"))
        environment = json.loads(
            (output / "environment.json").read_text(encoding="utf-8")
        )
        assert manifest["schema"] == "nq.install_first_run.campaign.v1"
        assert manifest["status"] == "blocked"
        assert manifest["blocker"]["code"] == "source_archive_missing"
        assert manifest["time_to_first_meaningful_host_result_ms"] is None
        assert manifest["observations"]["missing_dependency"]["status"] == "observed"
        assert environment["inherited_environment_variable_count"] == 0
        effective = environment["effective_environment"]
        assert effective["HOME"].startswith("/tmp/nq-first-run-")
        assert effective["CARGO_HOME"].startswith(effective["HOME"])
        assert effective["RUSTUP_HOME"].startswith(effective["HOME"])
        assert effective["PYTHONDONTWRITEBYTECODE"] == "1"
        forbidden = {
            "NQ_CONFIG",
            "NQ_DB",
            "RUSTFLAGS",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "SSH_AUTH_SOCK",
        }
        assert forbidden.isdisjoint(effective)
        assert not (output / "__pycache__").exists()
        assert (output / "removal-reset-plan.json").is_file()

        existing_output = root / "existing"
        existing_output.mkdir()
        refused = subprocess.run(
            [
                sys.executable,
                str(harness_path),
                "--track",
                "source-archive",
                "--profile",
                "suite-minimal",
                "--source-archive",
                str(ordinary_archive),
                "--output",
                str(existing_output),
            ],
            cwd="/tmp",
            env={
                "PATH": "/usr/local/bin:/usr/bin:/bin",
                "PYTHONDONTWRITEBYTECODE": "1",
            },
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=30,
        )
        assert refused.returncode == 3
        assert "output already exists" in refused.stderr

    if os.environ.get("PYTHONDONTWRITEBYTECODE") != "1":
        print(
            "warning: invoke with PYTHONDONTWRITEBYTECODE=1 to keep the tree clean",
            file=sys.stderr,
        )
    print("install first-run campaign self-test: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
