//! Build script: bake the current git commit hash into the binary so the
//! liveness artifact (and downstream FLEET_INDEX consumers) can identify
//! which build wrote a given artifact.
//!
//! `NQ_BUILD_COMMIT` is exposed via `option_env!` from `liveness.rs`. When
//! the build environment does not produce a value (e.g. release tarball
//! without a .git directory, or a sandbox where `git` is not on PATH),
//! the env var is unset and consumers see `None` — honest absence beats
//! fabricated identity.

use std::process::Command;

fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(c) = commit {
        println!("cargo:rustc-env=NQ_BUILD_COMMIT={}", c);
    }

    // Re-run when HEAD or refs change so the baked commit stays accurate
    // across builds without forcing a clean. The paths are relative to
    // this crate's manifest dir, walking up to the workspace root.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
    println!("cargo:rerun-if-env-changed=NQ_BUILD_COMMIT");
}
