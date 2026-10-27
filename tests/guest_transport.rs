use e2evm::{
    guest,
    profile::{Profile, Store},
    safe_fs,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    _temporary: tempfile::TempDir,
    store: Store,
    mailbox: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::open(temporary.path().join("runtime")).unwrap();
        store
            .create(&Profile::new("test", "Test", 2, 2048).unwrap())
            .unwrap();
        let mailbox = store.dir("test").unwrap().join("mailbox");
        for directory in [
            &mailbox,
            &mailbox.join("commands"),
            &mailbox.join("replies"),
        ] {
            safe_fs::private_dir(directory).unwrap();
        }
        Self {
            _temporary: temporary,
            store,
            mailbox,
        }
    }

    fn start_worker(&self, crash_reply: &str) -> Worker {
        let _ = fs::remove_file(self.mailbox.join("heartbeat.json"));
        // Only adapt the guest account's home to this fixture. The real worker
        // serves the real mailbox and executes actual subprocesses unchanged.
        // Fault injection can end the worker after execution but before a reply.
        let bootstrap = r#"
import importlib.util, os, pathlib, subprocess, sys
spec = importlib.util.spec_from_file_location('worker', sys.argv[1])
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)
mailbox = pathlib.Path(sys.argv[2])
real_popen = subprocess.Popen
def launch(*args, **kwargs):
    kwargs['cwd'] = str(mailbox)
    kwargs['env']['HOME'] = str(mailbox)
    return real_popen(*args, **kwargs)
worker.subprocess.Popen = launch
if os.geteuid() == 0:
    worker.os.geteuid = lambda: 1
write_reply = worker.write_reply
def publish(path, value):
    if path.name == sys.argv[3]:
        os._exit(73)
    write_reply(path, value)
worker.write_reply = publish
worker.serve(mailbox)
"#;
        let child = Command::new("python3")
            .args(["-B", "-u", "-c", bootstrap])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("guest/worker.py"))
            .arg(&self.mailbox)
            .arg(crash_reply)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut worker = Worker(child);
        wait_for(|| {
            assert!(
                worker.0.try_wait().unwrap().is_none(),
                "Worker exited early"
            );
            guest::online(&self.store, "test")
        });
        worker
    }

    fn request(&self, id: &str, request: Value) {
        safe_fs::Directory::open(&self.mailbox.join("commands"))
            .unwrap()
            .write_new(
                &format!("{id}.json"),
                &serde_json::to_vec(&request).unwrap(),
            )
            .unwrap();
    }

    fn reply(&self, id: &str) -> Value {
        let path = self.mailbox.join("replies").join(format!("{id}.json"));
        wait_for(|| path.exists());
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn execute(&self, argv: &[&str], timeout: u64) -> Value {
        guest::execute(
            &self.store,
            "test",
            &argv
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>(),
            timeout,
        )
        .unwrap()
    }
}

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "Timed out waiting for fixture");
        thread::sleep(Duration::from_millis(20));
    }
}

fn timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[test]
fn rust_to_worker_preserves_arguments_results_and_timeout_recovery() {
    let fixture = Fixture::new();
    let _worker = fixture.start_worker("");
    let arguments = [
        "",
        "two words",
        "quotes'\"",
        "한글 🧇",
        "line\nbreak",
        "$(touch injected)",
    ];
    let mut argv = vec![
        "python3",
        "-c",
        "import json,sys; print(json.dumps(sys.argv[1:], ensure_ascii=False))",
    ];
    argv.extend(arguments);
    let result = fixture.execute(&argv, 3);
    assert_eq!(result["ok"], true);
    assert_eq!(result["target"], "guest");
    let received: Vec<String> = serde_json::from_str(result["output"].as_str().unwrap()).unwrap();
    assert_eq!(received, arguments);
    assert!(!fixture.mailbox.join("injected").exists());
    assert_eq!(
        fs::read_dir(fixture.mailbox.join("replies"))
            .unwrap()
            .count(),
        0
    );

    let failed = fixture.execute(
        &[
            "python3",
            "-c",
            "import sys; print('failure detail'); sys.exit(7)",
        ],
        3,
    );
    assert_eq!(failed["ok"], false);
    assert_eq!(failed["exit_code"], 7);
    assert_eq!(failed["output"], "failure detail\n");

    let timed_out = fixture.execute(&["python3", "-c", "import time; time.sleep(10)"], 1);
    assert_eq!(timed_out["ok"], false);
    assert_eq!(timed_out["timed_out"], true);
    let following = fixture.execute(&["python3", "-c", "print('still serving')"], 3);
    assert_eq!(following["ok"], true);
    assert_eq!(following["output"], "still serving\n");
}

#[test]
fn serve_consumes_expired_and_mismatched_requests_without_executing() {
    let fixture = Fixture::new();
    let command = [
        "python3",
        "-c",
        "from pathlib import Path; Path('unexpected').touch()",
    ];
    fixture.request(
        "expired",
        json!({"version":1,"id":"expired","expires_at":0,"argv":command}),
    );
    fixture.request(
        "mismatch",
        json!({"version":1,"id":"different","expires_at":timestamp()+60.0,"argv":command}),
    );
    let _worker = fixture.start_worker("");
    for (id, error) in [("expired", "expired"), ("mismatch", "ID mismatch")] {
        let reply = fixture.reply(id);
        assert_eq!(reply["id"], id);
        assert_eq!(reply["ok"], false);
        assert!(reply["error"].as_str().unwrap().contains(error));
        assert!(
            !fixture
                .mailbox
                .join("commands")
                .join(format!("{id}.json"))
                .exists()
        );
    }
    assert!(!fixture.mailbox.join("unexpected").exists());
}

#[test]
fn consumed_command_is_not_replayed_after_worker_dies_before_reply() {
    let fixture = Fixture::new();
    let mut first = fixture.start_worker("once.json");
    fixture.request("once", json!({"version":1,"id":"once","expires_at":timestamp()+60.0,"timeout":3,
        "argv":["python3","-c","from pathlib import Path; assert not Path('commands/once.json').exists(); p=Path('executions'); p.write_text(p.read_text()+'run' if p.exists() else 'run')"]}));
    wait_for(|| first.0.try_wait().unwrap().is_some());
    assert_eq!(first.0.wait().unwrap().code(), Some(73));
    assert!(!fixture.mailbox.join("commands/once.json").exists());
    assert!(!fixture.mailbox.join("replies/once.json").exists());
    let before = fs::read(fixture.mailbox.join("executions")).unwrap();
    assert_eq!(before, b"run");
    let _restarted = fixture.start_worker("");
    let following = fixture.execute(&["python3", "-c", "print('restarted')"], 3);
    assert_eq!(following["ok"], true);
    assert_eq!(
        fs::read(fixture.mailbox.join("executions")).unwrap(),
        before
    );
    assert!(!fixture.mailbox.join("replies/once.json").exists());
}

#[test]
fn rust_rejects_a_reply_with_another_request_id() {
    let fixture = Fixture::new();
    fs::write(
        fixture.mailbox.join("heartbeat.json"),
        serde_json::to_vec(&json!({"timestamp":timestamp()})).unwrap(),
    )
    .unwrap();
    let mailbox = fixture.mailbox.clone();
    let responder = thread::spawn(move || {
        let mut request = None;
        wait_for(|| {
            request = fs::read_dir(mailbox.join("commands"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "json")
                });
            request.is_some()
        });
        let filename = request.unwrap().file_name().unwrap().to_owned();
        safe_fs::Directory::open(&mailbox.join("replies"))
            .unwrap()
            .write_new(
                filename.to_str().unwrap(),
                br#"{"id":"another-request","ok":true,"output":"wrong"}"#,
            )
            .unwrap();
    });
    let result = guest::execute(&fixture.store, "test", &["unused".to_owned()], 1);
    responder.join().unwrap();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("reply ID mismatch")
    );
}
