#![cfg(unix)]

use e2evm::{bridge, profile::Store};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn store() -> (tempfile::TempDir, Store) {
    // macOS Unix socket paths are limited to 104 bytes.
    let temp = tempfile::Builder::new()
        .prefix("e2evm-bridge-")
        .tempdir_in("/tmp")
        .unwrap();
    let store = Store::open(temp.path().join("store")).unwrap();
    (temp, store)
}

fn listener(store: &Store) -> UnixListener {
    let path = store.root.join("control.sock");
    let listener = UnixListener::bind(&path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    listener
}

fn exchange(
    reply: impl FnOnce(Value) -> Vec<u8> + Send + 'static,
) -> anyhow::Result<Option<Value>> {
    let (_temp, store) = store();
    let listener = listener(&store);
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut client = loop {
            match listener.accept() {
                Ok((client, _)) => break client,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "Client did not connect");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("Cannot accept fixture connection: {error}"),
            }
        };
        client.set_nonblocking(false).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = String::new();
        BufReader::new(client.try_clone().unwrap())
            .read_line(&mut request)
            .unwrap();
        let request: Value = serde_json::from_str(&request).unwrap();
        assert_eq!(request["version"], 1);
        assert_eq!(request["profile"], "test");
        assert_eq!(request["action"], "status");
        assert_eq!(request["payload"], json!({}));
        client.write_all(&reply(request)).unwrap();
    });
    let response = bridge::request_if_running(&store, "test", "status", json!({}));
    server.join().unwrap();
    response
}

#[test]
fn absent_and_stale_workspaces_report_stopped_through_the_cli() {
    let (_temp, store) = store();
    store
        .create(&e2evm::profile::Profile::new("test", "Test", 2, 2048).unwrap())
        .unwrap();
    for stale in [false, true] {
        if stale {
            drop(listener(&store));
        }
        let reply = Command::new(env!("CARGO_BIN_EXE_e2evm"))
            .env("E2EVM_HOME", &store.root)
            .args(["status", "test"])
            .output()
            .unwrap();
        assert!(
            reply.status.success(),
            "{}",
            String::from_utf8_lossy(&reply.stderr)
        );
        let value: Value = serde_json::from_slice(&reply.stdout).unwrap();
        assert_eq!(value["state"], "stopped");
        assert_eq!(value["guest_online"], false);
        assert!(bridge::request(&store, "test", "start", json!({})).is_err());
    }
}

#[test]
fn authenticated_response_keeps_its_payload() {
    let value = exchange(|request| {
        let mut bytes = serde_json::to_vec(&json!({
            "id":request["id"], "ok":true, "state":"running"
        }))
        .unwrap();
        bytes.push(b'\n');
        bytes
    })
    .unwrap()
    .unwrap();
    assert_eq!(value["state"], "running");
}

#[test]
fn mismatched_reply_ids_are_not_accepted() {
    let error = exchange(|_| b"{\"id\":\"other\",\"ok\":true}\n".to_vec()).unwrap_err();
    assert!(error.to_string().contains("ID mismatch"));
}

#[test]
fn connected_failures_are_never_reclassified_as_stopped() {
    for reply in [
        b"".to_vec(),
        b"{\"ok\":true}".to_vec(),
        b"not-json\n".to_vec(),
    ] {
        assert!(exchange(move |_| reply).is_err());
    }
    let error = exchange(|request| {
        format!(
            "{}\n",
            json!({"id":request["id"],"ok":false,"error":"VM busy"})
        )
        .into_bytes()
    })
    .unwrap_err();
    assert_eq!(error.to_string(), "VM busy");
}

#[test]
fn unsafe_socket_paths_are_errors_even_without_a_listener() {
    let (_temp, store) = store();
    let path = store.root.join("control.sock");
    fs::write(&path, b"not a socket").unwrap();
    assert!(bridge::request_if_running(&store, "test", "status", json!({})).is_err());
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(store.root.join("missing"), &path).unwrap();
    assert!(bridge::request_if_running(&store, "test", "status", json!({})).is_err());
    fs::remove_file(&path).unwrap();
    drop(listener(&store));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(bridge::request_if_running(&store, "test", "status", json!({})).is_err());
}
