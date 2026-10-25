use harborvm::{
    assistant::{self, RunLimits},
    profile::{Profile, Store},
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

struct Fixture {
    _temp: tempfile::TempDir,
    store: Store,
}
impl Fixture {
    fn new(script: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let program = temp.path().join("codex-fixture");
        fs::write(&program, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let store = Store::open(temp.path().join("store")).unwrap();
        let mut profile = Profile::new("test-vm", "Test VM", 2, 2048).unwrap();
        profile.assistant_program = Some(program.to_string_lossy().into_owned());
        store.create(&profile).unwrap();
        Self { _temp: temp, store }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "assistant_subprocess", "--nocapture"])
            .env("HARBORVM_ASSISTANT_TEST_ROOT", &self.store.root)
            .env(
                "HARBORVM_ASSISTANT_TEST_PROMPT",
                "Explain $(touch INJECTED); `touch INJECTED` & < > ' \" 한글",
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
    fn work(&self) -> std::path::PathBuf {
        self.store.dir("test-vm").unwrap().join("assistant-work")
    }
}

// A separate process exercises signal handling without changing the test harness's signals.
#[test]
fn assistant_subprocess() {
    let Some(root) = std::env::var_os("HARBORVM_ASSISTANT_TEST_ROOT") else {
        return;
    };
    let store = Store::open(root.into()).unwrap();
    let limits = RunLimits {
        timeout: Duration::from_millis(
            std::env::var("HARBORVM_ASSISTANT_TEST_TIMEOUT")
                .unwrap_or("3000".into())
                .parse()
                .unwrap(),
        ),
        max_output_bytes: std::env::var("HARBORVM_ASSISTANT_TEST_CAP")
            .unwrap_or("65536".into())
            .parse()
            .unwrap(),
    };
    if let Err(error) = assistant::run_with_limits(
        &store,
        "test-vm",
        &std::env::var("HARBORVM_ASSISTANT_TEST_PROMPT").unwrap(),
        limits,
    ) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

#[test]
fn assistant_passes_prompt_as_data_and_emits_only_messages() {
    let fixture = Fixture::new(
        r#"
printf '%s\n' "$@" > args.txt
cat > prompt.txt
printf '%s\n' '{"type":"thread.started","thread_id":"private-id"}' '{"type":"item.completed","item":{"type":"agent_message","text":"Hello 한글"}}' '{"type":"turn.completed","usage":{"input_tokens":4}}'
"#,
    );
    let output = fixture.command().output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello 한글"));
    assert!(!stdout.contains("private-id"));
    let args = fs::read_to_string(fixture.work().join("args.txt")).unwrap();
    assert!(args.starts_with("exec\n--ignore-user-config\n--ephemeral\n--sandbox\nread-only\n"));
    assert!(args.ends_with("\n-\n"));
    let prompt = fs::read_to_string(fixture.work().join("prompt.txt")).unwrap();
    assert!(prompt.contains("$(touch INJECTED); `touch INJECTED`"));
    assert!(!fixture.work().join("INJECTED").exists());
    assert_eq!(
        fs::metadata(fixture.work()).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[test]
fn assistant_reports_missing_program_and_authentication_failure() {
    let fixture = Fixture::new("exit 0");
    let mut profile = fixture.store.load("test-vm").unwrap();
    profile.assistant_program = Some(
        fixture
            .work()
            .join("missing")
            .to_string_lossy()
            .into_owned(),
    );
    fixture.store.save(&profile).unwrap();
    let output = fixture.command().output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Cannot launch"));

    let fixture = Fixture::new(
        r#"cat >/dev/null
printf '%s\n' 'secret-debug-value' >&2
printf '%s\n' '{"type":"turn.failed","error":{"message":"401 Unauthorized token=secret-value"}}'
exit 7"#,
    );
    let output = fixture.command().output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("authentication"));
    assert!(!error.contains("secret-value"));
    assert!(!error.contains("secret-debug-value"));

    let fixture = Fixture::new("cat >/dev/null\nexit 23");
    let output = fixture.command().output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("23"));
}

#[test]
fn assistant_bounds_output_and_time() {
    let fixture = Fixture::new("cat >/dev/null\nwhile :; do printf '0123456789abcdef'; done");
    let output = fixture
        .command()
        .env("HARBORVM_ASSISTANT_TEST_CAP", "128")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output limit"));
    let fixture = Fixture::new("cat >/dev/null\nwhile :; do printf '0123456789abcdef' >&2; done");
    let output = fixture
        .command()
        .env("HARBORVM_ASSISTANT_TEST_CAP", "128")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output limit"));
    let fixture = Fixture::new("sleep 30");
    let start = Instant::now();
    let output = fixture
        .command()
        .env("HARBORVM_ASSISTANT_TEST_TIMEOUT", "100")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("timed out"));
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn assistant_cancellation_terminates_descendants() {
    let fixture =
        Fixture::new("trap '' TERM\ncat >/dev/null\nsleep 30 &\necho $! > child.pid\nwait");
    let child = fixture.command().spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let pid_file = fixture.work().join("child.pid");
    while !pid_file.exists() {
        assert!(Instant::now() < deadline, "fixture failed to start");
        std::thread::sleep(Duration::from_millis(10));
    }
    let descendant: i32 = fs::read_to_string(pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cancelled"));
    let deadline = Instant::now() + Duration::from_secs(3);
    while unsafe { libc::kill(descendant, 0) } == 0 {
        assert!(
            Instant::now() < deadline,
            "descendant survived cancellation"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn assistant_flushes_messages_before_exit() {
    use std::io::BufRead;
    let fixture = Fixture::new(
        r#"cat >/dev/null
printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"early response"}}'
sleep 30"#,
    );
    let mut child = fixture.command().spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            if line.unwrap() == "early response" {
                sender.send(()).unwrap();
                break;
            }
        }
    });
    let received = receiver.recv_timeout(Duration::from_secs(2));
    let still_running = child.try_wait().unwrap().is_none();
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    let output = child.wait_with_output().unwrap();
    reader.join().unwrap();
    assert!(received.is_ok(), "assistant output was buffered until exit");
    assert!(still_running);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cancelled"));
}
