use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=10"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    let stripped = hash.strip_suffix("-dirty").unwrap_or(&hash);
    let dirty = hash.ends_with("-dirty");

    let release = std::env::var("PROFILE").map(|p| p == "release").unwrap_or(false);
    let release_dirty_override = std::env::var("ALLOW_DIRTY_RELEASE").map(|v| v == "true").unwrap_or(false);

    if dirty && release && !release_dirty_override {
        panic!("Cannot build a release with a dirty git state. If you really want to do this, set the ALLOW_DIRTY_RELEASE environment variable to true.");
    }

    println!("cargo:rustc-env=GIT_HASH={stripped}");
    println!("cargo:rustc-env=GIT_DIRTY={}", dirty);
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
}
