use std::process::Command;

fn main() {
    let described = Command::new("git")
        .args(["describe", "--long", "--tags", "--always", "--match", "v*"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .and_then(|s| s.strip_prefix("v").map(|s| s.to_owned()));

    let version = described.unwrap_or_else(|| "0.0.0-unknown".into());

    println!("cargo:rustc-env=GIT_VERSION={version}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");
}
