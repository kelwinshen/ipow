//! The commit the node is built from, for its first log line: which code a
//! server runs is then read from its log.

use std::process::Command;

fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let dirty = Command::new("git").args(["status", "--porcelain", "--untracked-files=no"]).output().ok().is_some_and(|o| !o.stdout.is_empty());
    println!("cargo:rustc-env=IPOW_COMMIT={commit}{}", if dirty { "+changes" } else { "" });
    println!("cargo:rerun-if-changed=../../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../../.git/refs/heads");
    println!("cargo:rerun-if-changed=../../../.git/index");
}
