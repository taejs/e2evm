//! The local Codex adapter. Assistant text is output, never a guest command.
use crate::{Context, Result, bail, profile::Store, safe_fs};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::{fs::PermissionsExt, io::AsRawFd, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicI32, Ordering},
    },
    time::{Duration, Instant},
};

const MAX_PROMPT_BYTES: usize = 64 * 1024;
static ACTIVE: Mutex<()> = Mutex::new(());
static CANCELLED: AtomicI32 = AtomicI32::new(0);

/// Bounds apply to the entire subprocess, including stderr and partial JSON lines.
#[derive(Clone, Copy)]
pub struct RunLimits {
    pub timeout: Duration,
    pub max_output_bytes: usize,
}
impl Default for RunLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            max_output_bytes: 8 * 1024 * 1024,
        }
    }
}

pub fn run(store: &Store, id: &str, prompt: &str) -> Result<()> {
    run_with_limits(store, id, prompt, RunLimits::default())
}

/// Runs one turn, flushing completed assistant messages to stdout as they arrive.
/// Signal handlers are process-wide, so concurrent calls in one process are refused.
pub fn run_with_limits(store: &Store, id: &str, prompt: &str, limits: RunLimits) -> Result<()> {
    if prompt.trim().is_empty() {
        bail!("Assistant prompt is empty");
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        bail!("Assistant prompt exceeds 64 KiB");
    }
    if limits.timeout.is_zero() || limits.max_output_bytes == 0 {
        bail!("Assistant limits must be positive");
    }
    let _active = ACTIVE
        .try_lock()
        .map_err(|_| anyhow::anyhow!("An assistant turn is already running in this process"))?;
    let profile = store.load(id)?;
    let work = store.dir(id)?.join("assistant-work");
    safe_fs::private_dir(&work)?;
    let _lock = safe_fs::Lock::acquire(&work.join(".turn.lock"))?;
    let program = resolve_program(profile.assistant_program.as_deref())?;
    let context = json!({"profile_id": profile.id, "profile_name": profile.name});
    let input = format!(
        "You are the advisory assistant in HarborVM, a local virtual machine workspace. \
         Only this profile identity is supplied: {context}\n\
         No guest files, terminal output, clipboard, or credentials have been supplied. \
         Do not claim to inspect or operate this VM. Explain commands as suggestions for the user; \
         your response is displayed as text and is never executed by HarborVM.\n\nUser request:\n{prompt}\n"
    );
    let _signals = SignalGuard::install()?;
    let child = Command::new(&program)
        .args(["exec", "--ignore-user-config", "--ephemeral", "--sandbox", "read-only", "--skip-git-repo-check", "--json", "--color", "never", "-C"])
        .arg(&work).arg("-").current_dir(&work)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .process_group(0).spawn()
        .context("Cannot launch local Codex CLI; install Codex or set this profile's assistant_program to its executable path")?;
    let mut process = ProcessGroup(child);
    let mut stdin = process.0.stdin.take();
    let mut stdout = process
        .0
        .stdout
        .take()
        .context("Missing assistant stdout")?;
    let mut stderr = process
        .0
        .stderr
        .take()
        .context("Missing assistant stderr")?;
    set_nonblocking(stdin.as_ref().context("Missing assistant stdin")?)?;
    set_nonblocking(&stdout)?;
    set_nonblocking(&stderr)?;
    let started = Instant::now();
    let mut sent = 0;
    let mut total = 0;
    let mut events = Events::default();
    let mut out_eof = false;
    let mut err_eof = false;
    let mut had_stderr = false;
    loop {
        if CANCELLED.load(Ordering::Relaxed) != 0 {
            bail!("Assistant turn cancelled");
        }
        if started.elapsed() >= limits.timeout {
            bail!("Assistant turn timed out");
        }
        if let Some(pipe) = stdin.as_mut() {
            match pipe.write(&input.as_bytes()[sent..]) {
                Ok(count) => {
                    sent += count;
                    if sent == input.len() {
                        stdin = None;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                    stdin = None;
                }
                Err(error) => return Err(error).context("Cannot send assistant prompt"),
            }
        }
        if !out_eof {
            out_eof = pump(&mut stdout, &mut total, limits.max_output_bytes, |bytes| {
                events.receive(bytes)
            })?;
        }
        if !err_eof {
            err_eof = pump(&mut stderr, &mut total, limits.max_output_bytes, |_| {
                had_stderr = true;
                Ok(())
            })?;
        }
        if let Some(status) = process.0.try_wait()? {
            if out_eof && err_eof {
                events.finish()?;
                if !status.success() {
                    if had_stderr {
                        eprintln!(
                            "Codex diagnostic logs were withheld; they may contain private data."
                        );
                    }
                    bail!(
                        "Local Codex CLI exited with {status}; run `codex login status` to check authentication"
                    );
                }
                if events.messages == 0 {
                    bail!("Local Codex CLI returned no assistant message");
                }
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}
fn resolve_program(configured: Option<&str>) -> Result<PathBuf> {
    let name = configured.unwrap_or("codex");
    if name.is_empty() {
        bail!("assistant_program must name a Codex executable");
    }
    let path = Path::new(name);
    if path.components().count() > 1 {
        if !path.is_absolute() {
            bail!("assistant_program must be an absolute path or a program name on PATH");
        }
        return Ok(path.to_path_buf());
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths).filter(|path| path.is_absolute()) {
            let candidate = directory.join(name);
            if executable(&candidate) {
                return Ok(candidate);
            }
        }
    }
    if configured.is_none() {
        if let Some(home) = std::env::var_os("HOME") {
            let candidate = PathBuf::from(home).join(".local/bin/codex");
            if executable(&candidate) {
                return Ok(candidate);
            }
        }
    }
    bail!(
        "Cannot launch local Codex CLI: executable was not found on PATH; install Codex or set assistant_program to an absolute path"
    )
}

fn set_nonblocking(file: &impl AsRawFd) -> Result<()> {
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error()).context("Cannot configure assistant pipe");
    }
    Ok(())
}

fn pump(
    reader: &mut impl Read,
    total: &mut usize,
    limit: usize,
    mut receive: impl FnMut(&[u8]) -> Result<()>,
) -> Result<bool> {
    let mut bytes = [0; 8192];
    // Return to cancellation/timeout checks even when a producer never stops writing.
    for _ in 0..16 {
        match reader.read(&mut bytes) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                *total = total.saturating_add(count);
                if *total > limit {
                    bail!("Assistant exceeded its output limit");
                }
                receive(&bytes[..count])?;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Ok(false),
            Err(error) => return Err(error).context("Cannot read assistant output"),
        }
    }
    Ok(false)
}

#[derive(Default)]
struct Events {
    pending: Vec<u8>,
    messages: usize,
}
impl Events {
    fn receive(&mut self, bytes: &[u8]) -> Result<()> {
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<_> = self.pending.drain(..=end).collect();
            self.line(&line)?;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        if !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            self.line(&line)?;
        }
        Ok(())
    }
    fn line(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Ok(());
        }
        let event: Value = serde_json::from_slice(bytes).context(
            "Codex returned invalid JSON output; this adapter requires `codex exec --json`",
        )?;
        match event["type"].as_str() {
            Some("item.completed") if event["item"]["type"] == "agent_message" => {
                let text = event["item"]["text"]
                    .as_str()
                    .context("Codex assistant message has no text")?;
                let mut output = io::stdout().lock();
                output.write_all(text.as_bytes())?;
                if !text.ends_with('\n') {
                    output.write_all(b"\n")?;
                }
                output.flush()?;
                self.messages += 1;
            }
            Some("error" | "turn.failed") => {
                let message = event["message"]
                    .as_str()
                    .or_else(|| event["error"]["message"].as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                // Never relay free-form diagnostic strings: they can contain URLs, tokens, or prompts.
                let reason = if [
                    "401",
                    "unauthorized",
                    "authentication",
                    "not logged in",
                    "login",
                    "api key",
                ]
                .iter()
                .any(|word| message.contains(word))
                {
                    "authentication failed; run `codex login` in your terminal"
                } else if ["quota", "rate limit", "429", "usage limit"]
                    .iter()
                    .any(|word| message.contains(word))
                {
                    "usage limit reached; check your Codex account"
                } else if ["network", "connect", "timeout", "timed out", "dns"]
                    .iter()
                    .any(|word| message.contains(word))
                {
                    "connection failed; check network access and try again"
                } else {
                    "request failed; run Codex directly for private diagnostics"
                };
                bail!("Local Codex CLI {reason}");
            }
            _ => {}
        }
        Ok(())
    }
}

extern "C" fn cancel(signal: libc::c_int) {
    CANCELLED.store(signal, Ordering::Relaxed);
}
struct SignalGuard {
    previous: Vec<(libc::c_int, libc::sigaction)>,
}
impl SignalGuard {
    fn install() -> Result<Self> {
        CANCELLED.store(0, Ordering::Relaxed);
        let mut guard = Self {
            previous: Vec::new(),
        };
        for signal in [libc::SIGINT, libc::SIGTERM] {
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = cancel as *const () as usize;
            unsafe {
                libc::sigemptyset(&mut action.sa_mask);
            }
            let mut previous = unsafe { std::mem::zeroed() };
            if unsafe { libc::sigaction(signal, &action, &mut previous) } != 0 {
                return Err(io::Error::last_os_error())
                    .context("Cannot install assistant cancellation handler");
            }
            guard.previous.push((signal, previous));
        }
        Ok(guard)
    }
}
impl Drop for SignalGuard {
    fn drop(&mut self) {
        for (signal, action) in &self.previous {
            unsafe {
                libc::sigaction(*signal, action, std::ptr::null_mut());
            }
        }
    }
}
struct ProcessGroup(Child);
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let group = -(self.0.id() as libc::pid_t);
        unsafe {
            libc::kill(group, libc::SIGTERM);
        }
        // A child can exit while its descendants remain, so always finish group cleanup.
        std::thread::sleep(Duration::from_millis(200));
        unsafe {
            libc::kill(group, libc::SIGKILL);
        }
        let _ = self.0.wait();
    }
}
