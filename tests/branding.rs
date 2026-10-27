use std::{fs, path::Path, process::Command};

#[test]
fn environment_probe() {
    if std::env::var_os("E2EVM_BRANDING_PROBE").is_none() {
        return;
    }
    let root = e2evm::profile::Store::default_root().unwrap();
    let app = e2evm::bridge::executable().ok();
    println!("PROBE:{}", serde_json::json!({"root":root,"app":app}));
}
fn probe(
    home: &Path,
    new_home: Option<&Path>,
    legacy_home: Option<&Path>,
    new_app: Option<&Path>,
    legacy_app: Option<&Path>,
) -> serde_json::Value {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "environment_probe", "--nocapture"])
        .env("E2EVM_BRANDING_PROBE", "1")
        .env("HOME", home);
    for (name, value) in [
        ("E2EVM_HOME", new_home),
        ("HARBORVM_HOME", legacy_home),
        ("E2EVM_APP", new_app),
        ("HARBORVM_APP", legacy_app),
    ] {
        command.env_remove(name);
        if let Some(value) = value {
            command.env(name, value);
        }
    }
    let output = command.output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    serde_json::from_str(
        stdout
            .lines()
            .find_map(|line| line.strip_prefix("PROBE:"))
            .unwrap(),
    )
    .unwrap()
}
fn bundle(root: &Path, name: &str, executable: &str) -> std::path::PathBuf {
    let path = root.join(name);
    fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
    fs::write(path.join("Contents/MacOS").join(executable), b"fixture").unwrap();
    path
}
#[test]
fn new_home_precedes_legacy_and_default_uses_new_brand() {
    let temp = tempfile::tempdir().unwrap();
    let new = temp.path().join("new");
    let old = temp.path().join("old");
    assert_eq!(
        probe(temp.path(), Some(&new), Some(&old), None, None)["root"],
        new.to_str().unwrap()
    );
    assert_eq!(
        probe(temp.path(), None, Some(&old), None, None)["root"],
        old.to_str().unwrap()
    );
    assert_eq!(
        probe(temp.path(), None, None, None, None)["root"],
        temp.path()
            .join("Library/Application Support/E2EVM")
            .to_str()
            .unwrap()
    );
}
#[test]
fn new_app_precedes_legacy_and_bundle_executable_is_discovered() {
    let temp = tempfile::tempdir().unwrap();
    let new = bundle(temp.path(), "E2EVM.app", "E2EVMDesktop");
    let old = bundle(temp.path(), "HarborVM.app", "HarborVMDesktop");
    assert_eq!(
        probe(temp.path(), None, None, Some(&new), Some(&old))["app"],
        new.join("Contents/MacOS/E2EVMDesktop").to_str().unwrap()
    );
    assert_eq!(
        probe(temp.path(), None, None, None, Some(&old))["app"],
        old.join("Contents/MacOS/HarborVMDesktop").to_str().unwrap()
    );
    let renamed = bundle(temp.path(), "Custom.app", "HarborVMDesktop");
    assert_eq!(
        probe(temp.path(), None, None, Some(&renamed), None)["app"],
        renamed
            .join("Contents/MacOS/HarborVMDesktop")
            .to_str()
            .unwrap()
    );
    let direct = new.join("Contents/MacOS/E2EVMDesktop");
    assert_eq!(
        probe(temp.path(), None, None, Some(&direct), None)["app"],
        direct.to_str().unwrap()
    );
}
