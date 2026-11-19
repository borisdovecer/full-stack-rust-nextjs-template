//! Records the git commit the binary is built from. `GIT_COMMIT` wins, for builds outside a
//! git checkout (CI artifacts, deploys).

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=GIT_COMMIT");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    let commit = std::env::var("GIT_COMMIT")
        .ok()
        .filter(|c| !c.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short=12", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
        });
    println!(
        "cargo:rustc-env=APP_GIT_COMMIT={}",
        commit.unwrap_or_else(|| "unknown".into())
    );
}
