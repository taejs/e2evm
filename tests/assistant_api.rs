use harborvm::profile::{Profile, Store};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

struct Fixture {
    _temp: tempfile::TempDir,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("store")).unwrap();
        store
            .create(&Profile::new("test", "Test", 2, 2048).unwrap())
            .unwrap();
        Self { _temp: temp, store }
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_harborvm"));
        c.args(args)
            .env("HARBORVM_HOME", &self.store.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    }
    fn configure(&self, provider: &str, endpoint: &str) {
        let o = self
            .command(&[
                "profile",
                "connection",
                "test",
                provider,
                "--endpoint",
                endpoint,
                "--model",
                "fixture-model",
            ])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
    fn chat(&self) -> std::process::Output {
        let mut c = self.command(&["chat-json", "test"]).spawn().unwrap();
        c.stdin.take().unwrap().write_all(br#"{"messages":[{"role":"user","content":"first"},{"role":"assistant","content":"previous"},{"role":"user","content":"second"}],"api_key":"secret-fixture"}"#).unwrap();
        c.wait_with_output().unwrap()
    }
}
fn server(status: &str, body: &str) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut input = Vec::new();
        let mut b = [0; 1];
        while !input.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut b).unwrap();
            input.push(b[0]);
        }
        let header = String::from_utf8(input.clone()).unwrap();
        let size = header
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        let mut body = vec![0; size];
        stream.read_exact(&mut body).unwrap();
        input.extend(body);
        tx.send(String::from_utf8(input).unwrap()).unwrap();
        for chunk in response.as_bytes().chunks(7) {
            if stream.write_all(chunk).is_err() {
                break;
            }
        }
    });
    (url, rx, handle)
}
#[test]
fn ollama_stream_preserves_history_and_keeps_key_out_of_profile() {
    let f = Fixture::new();
    let (url, rx, h) = server(
        "200 OK",
        "{\"message\":{\"content\":\"Hello \"},\"done\":false}\n{\"message\":{\"content\":\"한글\"},\"done\":true}\n",
    );
    f.configure("ollama", &url);
    let o = f.chat();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(String::from_utf8(o.stdout).unwrap(), "Hello 한글");
    let request = rx.recv_timeout(Duration::from_secs(3)).unwrap();
    h.join().unwrap();
    assert!(request.starts_with("POST /api/chat "));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret-fixture")
    );
    let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["messages"][2]["content"], "previous");
    assert_eq!(body["messages"][3]["content"], "second");
    assert_eq!(body["model"], "fixture-model");
    assert_eq!(body["stream"], true);
    assert!(
        !std::fs::read_to_string(f.store.dir("test").unwrap().join("profile.json"))
            .unwrap()
            .contains("secret-fixture")
    );
}
#[test]
fn compatible_sse_and_model_discovery() {
    let f = Fixture::new();
    let (url, rx, h) = server(
        "200 OK",
        ": ping\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n",
    );
    f.configure("openai-compatible", &format!("{url}/v1"));
    let o = f.chat();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(o.stdout, b"ok");
    assert!(rx.recv().unwrap().starts_with("POST /v1/chat/completions "));
    h.join().unwrap();
    let (url, rx, h) = server(
        "200 OK",
        "{\"data\":[{\"id\":\"second\"},{\"id\":\"first\"}]}",
    );
    f.configure("openai-compatible", &format!("{url}/v1"));
    let o = f
        .command(&["assistant", "models", "test"])
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        json!([{"id":"first"},{"id":"second"}])
    );
    assert!(rx.recv().unwrap().starts_with("GET /v1/models "));
    h.join().unwrap();
}
#[test]
fn errors_are_redacted_and_truncated_stream_fails() {
    for (status, body) in [
        ("401 Unauthorized", "secret-fixture private-url"),
        ("200 OK", "{\"error\":\"secret-fixture\"}\n"),
        (
            "200 OK",
            "{\"message\":{\"content\":\"partial\"},\"done\":false}\n",
        ),
        ("200 OK", "private-url invalid json\n"),
    ] {
        let f = Fixture::new();
        let (url, rx, h) = server(status, body);
        f.configure("ollama", &url);
        let o = f.chat();
        assert!(!o.status.success());
        let error = String::from_utf8_lossy(&o.stderr);
        assert!(!error.contains("secret-fixture"));
        assert!(!error.contains("private-url"));
        rx.recv().unwrap();
        h.join().unwrap();
    }
}
#[test]
fn unsafe_endpoints_and_invalid_history_are_rejected() {
    let f = Fixture::new();
    for endpoint in [
        "http://example.com",
        "https://user:secret@example.com/v1",
        "https://example.com/v1?key=secret",
        "file:///tmp/foo",
    ] {
        let o = f
            .command(&[
                "profile",
                "connection",
                "test",
                "ollama",
                "--endpoint",
                endpoint,
            ])
            .output()
            .unwrap();
        assert!(!o.status.success());
        assert!(!String::from_utf8_lossy(&o.stderr).contains("secret"));
    }
    let mut c = f.command(&["chat-json", "test"]).spawn().unwrap();
    c.stdin
        .take()
        .unwrap()
        .write_all(br#"{"messages":[{"role":"system","content":"no"}]}"#)
        .unwrap();
    assert!(!c.wait_with_output().unwrap().status.success());
}

#[test]
fn streaming_is_visible_before_completion_and_cancel_closes_connection() {
    let f = Fixture::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    f.configure(
        "ollama",
        &format!("http://{}", listener.local_addr().unwrap()),
    );
    let (sent, received) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut head = Vec::new();
        let mut byte = [0; 1];
        while !head.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        let headers = String::from_utf8(head).unwrap();
        let size = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse::<usize>().unwrap())
            })
            .unwrap();
        socket.read_exact(&mut vec![0; size]).unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n{\"message\":{\"content\":\"early\"},\"done\":false}\n").unwrap();
        sent.send(socket.read(&mut byte).unwrap_or(0)).unwrap();
    });
    let mut child = f.command(&["chat-json", "test"]).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"messages":[{"role":"user","content":"hello"}]}"#)
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut b = [0; 5];
        stdout.read_exact(&mut b).unwrap();
        tx.send(b).unwrap();
    });
    let early = rx.recv_timeout(Duration::from_secs(3));
    let running = child.try_wait().unwrap().is_none();
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(early.unwrap(), *b"early");
    assert!(running);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cancelled"));
    assert_eq!(received.recv_timeout(Duration::from_secs(3)).unwrap(), 0);
    reader.join().unwrap();
    server.join().unwrap();
}

#[test]
fn redirects_are_not_followed_and_keys_use_named_environment_only() {
    let f = Fixture::new();
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    f.configure(
        "ollama",
        &format!("http://{}", listener.local_addr().unwrap()),
    );
    let target_url = format!("http://{}/credential-trap", target.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut b = [0; 8192];
        let _ = socket.read(&mut b).unwrap();
        socket
            .write_all(
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: {target_url}\r\nContent-Length: 0\r\n\r\n"
                )
                .as_bytes(),
            )
            .unwrap();
    });
    let output = f.chat();
    server.join().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("redirect refused"));
    assert_eq!(
        target.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let (url, rx, h) = crate::server("200 OK", "{\"models\":[{\"name\":\"fixture-model\"}]}");
    let o = f
        .command(&[
            "profile",
            "connection",
            "test",
            "ollama",
            "--endpoint",
            &url,
            "--key-env",
            "HARBORVM_FIXTURE_TOKEN",
        ])
        .output()
        .unwrap();
    assert!(o.status.success());
    let o = f
        .command(&["assistant", "models", "test"])
        .env("HARBORVM_FIXTURE_TOKEN", "named-env-token")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(
        rx.recv()
            .unwrap()
            .to_ascii_lowercase()
            .contains("authorization: bearer named-env-token")
    );
    h.join().unwrap();
    let o = f
        .command(&["assistant", "models", "test"])
        .env_remove("HARBORVM_FIXTURE_TOKEN")
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("environment variable is unavailable"));
}

#[test]
fn oversized_input_and_unterminated_stream_line_are_bounded() {
    let f = Fixture::new();
    let input = json!({"messages":[{"role":"user","content":"x".repeat(65536)}]}).to_string();
    let mut child = f.command(&["chat-json", "test"]).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("64 KiB"));
    let (url, rx, h) = server("200 OK", &"x".repeat(256 * 1024 + 1));
    f.configure("ollama", &url);
    let output = f.chat();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("256 KiB"));
    rx.recv().unwrap();
    h.join().unwrap();
}

#[test]
fn codex_json_entry_point_includes_prior_turns() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let executable = f._temp.path().join("codex-fixture");
    std::fs::write(&executable, "#!/bin/sh\ncat > received.txt\nprintf '%s\\n' '{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"reply\"}}'\n").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut profile = f.store.load("test").unwrap();
    profile.assistant_program = Some(executable.to_str().unwrap().into());
    f.store.save(&profile).unwrap();
    let mut child = f.command(&["chat-json", "test"]).spawn().unwrap();
    child.stdin.take().unwrap().write_all(br#"{"messages":[{"role":"user","content":"first"},{"role":"assistant","content":"prior answer"},{"role":"user","content":"followup"}]}"#).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let prompt = std::fs::read_to_string(
        f.store
            .dir("test")
            .unwrap()
            .join("assistant-work/received.txt"),
    )
    .unwrap();
    assert!(prompt.contains("prior answer"));
    assert!(prompt.contains("followup"));
}
