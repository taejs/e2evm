use harborvm::{profile::{Profile, Store}, safe_fs};
#[test]
fn profiles_round_trip_and_never_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("store")).unwrap();
    let profile = Profile::new("ubuntu", "Ubuntu", 2, 2048).unwrap();
    store.create(&profile).unwrap();
    assert!(store.create(&profile).is_err());
    assert_eq!(store.load("ubuntu").unwrap().name, "Ubuntu");
    assert_eq!(store.list().unwrap().len(), 1);
    for id in ["../other", "", "A", "-bad", "a/b"] { assert!(store.load(id).is_err()); }
}
#[test]
fn links_and_oversized_files_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("store")).unwrap();
    std::os::unix::fs::symlink(temp.path(), store.root.join("profiles/escape")).unwrap();
    assert!(store.load("escape").is_err());
    let file = temp.path().join("data"); std::fs::write(&file, b"12345").unwrap();
    assert!(safe_fs::read(&file, 4).is_err());
    let link = temp.path().join("link"); std::os::unix::fs::symlink(file, &link).unwrap();
    assert!(safe_fs::read(&link, 10).is_err());
}
#[test]
fn exclusive_locks_release_on_drop() {
    let temp = tempfile::tempdir().unwrap(); let path = temp.path().join("lock");
    let lock = safe_fs::Lock::acquire(&path).unwrap();
    assert!(safe_fs::Lock::acquire(&path).is_err());
    drop(lock); assert!(safe_fs::Lock::acquire(&path).is_ok());
}
