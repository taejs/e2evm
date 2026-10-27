use harborvm::{
    profile::{Profile, Store},
    safe_fs,
};
#[test]
fn profiles_round_trip_and_never_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("store")).unwrap();
    let profile = Profile::new("ubuntu", "Ubuntu", 2, 2048).unwrap();
    store.create(&profile).unwrap();
    assert!(store.create(&profile).is_err());
    assert_eq!(store.load("ubuntu").unwrap().name, "Ubuntu");
    assert_eq!(store.list().unwrap().len(), 1);
    for id in ["../other", "", "A", "-bad", "a/b"] {
        assert!(store.load(id).is_err());
    }
}

#[test]
fn old_profiles_keep_defaults_and_optional_dependencies_are_validated() {
    use harborvm::profile::{GuestOS, SetupOptions};
    let legacy = r#"{"version":1,"id":"old","name":"Old","cpus":2,"memory_mib":2048,"disk_gib":24,"ready":false}"#;
    let profile: Profile = serde_json::from_str(legacy).unwrap();
    assert_eq!(profile.guest_os, GuestOS::Linux);
    assert!(profile.setup.playwright && profile.setup.desktop);
    let mut setup = SetupOptions {
        browser: false,
        ..SetupOptions::default()
    };
    assert!(setup.validate().is_err());
    setup.playwright = false;
    assert!(setup.validate().is_ok());
    setup = SetupOptions {
        desktop: false,
        browser: false,
        python: false,
        node: false,
        playwright: false,
    };
    assert!(setup.validate().is_ok());
    let mut mac = profile;
    mac.guest_os = GuestOS::Macos;
    assert!(mac.validate().is_err());
    mac.memory_mib = 4096;
    mac.disk_gib = 64;
    assert!(mac.validate().is_ok());
}
#[test]
fn links_and_oversized_files_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("store")).unwrap();
    std::os::unix::fs::symlink(temp.path(), store.root.join("profiles/escape")).unwrap();
    assert!(store.load("escape").is_err());
    let file = temp.path().join("data");
    std::fs::write(&file, b"12345").unwrap();
    assert!(safe_fs::read(&file, 4).is_err());
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(file, &link).unwrap();
    assert!(safe_fs::read(&link, 10).is_err());
}
#[test]
fn exclusive_locks_release_on_drop() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("lock");
    let lock = safe_fs::Lock::acquire(&path).unwrap();
    assert!(safe_fs::Lock::acquire(&path).is_err());
    drop(lock);
    assert!(safe_fs::Lock::acquire(&path).is_ok());
}

#[test]
fn metadata_updates_preserve_settings_and_readiness_without_taking_vm_lease() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("store")).unwrap();
    store
        .create(&Profile::new("test", "Test", 2, 2048).unwrap())
        .unwrap();
    let _vm = safe_fs::Lock::acquire(&store.dir("test").unwrap().join("profile.lock")).unwrap();
    let stale = store.load("test").unwrap();
    store
        .update("test", |p| p.assistant_program = Some("new-codex".into()))
        .unwrap();
    store.update(&stale.id, |p| p.ready = true).unwrap();
    assert_eq!(
        store.load("test").unwrap().assistant_program.as_deref(),
        Some("new-codex")
    );
    store
        .update("test", |p| {
            p.assistant_program = Some("another-codex".into())
        })
        .unwrap();
    assert!(store.load("test").unwrap().ready);
    let metadata =
        safe_fs::Lock::acquire(&store.dir("test").unwrap().join("metadata.lock")).unwrap();
    assert!(store.update("test", |p| p.ready = false).is_err());
    drop(metadata);
    assert!(store.load("test").unwrap().ready);
}

#[test]
fn pinned_directory_survives_guest_path_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let inbox = base.join("inbox");
    let outside = base.join("outside");
    std::fs::create_dir(&inbox).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let directory = safe_fs::Directory::open(&inbox).unwrap();
    std::fs::rename(&inbox, base.join("old")).unwrap();
    std::os::unix::fs::symlink(&outside, &inbox).unwrap();
    directory.write_new("request.json", b"safe").unwrap();
    assert!(!outside.join("request.json").exists());
    assert_eq!(directory.read("request.json", 100).unwrap(), b"safe");
    assert!(safe_fs::Directory::open(&inbox).is_err());
}
