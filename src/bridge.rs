//! The platform bridge owns framework objects; Rust owns its request contract.
use crate::{Context, Result, bail, profile::Store};
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::os::unix::{
    fs::{FileTypeExt, MetadataExt},
    net::UnixStream,
};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn request(store: &Store, id: &str, action: &str, payload: Value) -> Result<Value> {
    crate::profile::validate_id(id)?;
    #[cfg(unix)]
    {
        let path = store.root.join("control.sock");
        let metadata =
            fs::symlink_metadata(&path).context("Workspace is not running; run harborvm app")?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            bail!("Unsafe workspace control socket");
        }
        let mut socket =
            UnixStream::connect(path).context("Cannot connect to workspace; run harborvm app")?;
        socket.set_read_timeout(Some(Duration::from_secs(120)))?;
        socket.set_write_timeout(Some(Duration::from_secs(10)))?;
        let request_id = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        );
        let mut bytes = serde_json::to_vec(
            &json!({"version":1,"id":request_id,"profile":id,"action":action,"payload":payload}),
        )?;
        if bytes.len() > 65535 {
            bail!("Bridge request exceeds 64 KiB");
        }
        bytes.push(b'\n');
        socket.write_all(&bytes)?;
        let mut response = Vec::new();
        BufReader::new(socket).take(32 * 1024 * 1024 + 1).read_until(b'\n', &mut response).context("Bridge reply unavailable; operation outcome may be unknown. Query status before retrying")?;
        if response.len() > 32 * 1024 * 1024 || response.last() != Some(&b'\n') {
            bail!("Incomplete or oversized bridge reply; query status before retrying");
        }
        let value: Value = serde_json::from_slice(&response)?;
        if value["id"] != request_id {
            bail!("Bridge reply ID mismatch");
        }
        if value["ok"] != true {
            bail!(
                "{}",
                value["error"]
                    .as_str()
                    .unwrap_or("Platform operation failed")
            );
        }
        Ok(value)
    }
    #[cfg(not(unix))]
    {
        let _ = (store, action, payload);
        bail!("The VM workspace requires macOS");
    }
}

pub fn executable() -> Result<PathBuf> {
    let current = std::env::current_exe()?;
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("HARBORVM_APP") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(parent) = current.parent() {
        candidates.push(parent.join("HarborVMDesktop"));
        candidates.push(parent.join("HarborVM.app"));
        candidates.push(parent.join("../../dist/HarborVM.app"));
    }
    candidates
        .into_iter()
        .map(|path| {
            if path.extension().is_some_and(|ext| ext == "app") {
                path.join("Contents/MacOS/HarborVMDesktop")
            } else {
                path
            }
        })
        .find(|path| path.is_file())
        .context(
            "Cannot find HarborVM.app; build it with scripts/build-macos.sh or set HARBORVM_APP",
        )
}

pub fn launch(store: &Store, activate: bool) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("The VM workspace requires macOS on Apple Silicon");
    }
    let executable = executable()?;
    let current = std::env::current_exe()?;
    let socket = store.root.join("control.sock");
    #[cfg(unix)]
    if UnixStream::connect(&socket).is_ok() {
        if activate {
            request(store, "workspace", "activate", json!({}))?;
        }
        return Ok(());
    }
    let mut command = Command::new(executable);
    if !activate {
        command.arg("--background");
    }
    let mut child = command
        .arg("--home")
        .arg(&store.root)
        .arg("--cli")
        .arg(current)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .context("Cannot launch native workspace")?;
    for _ in 0..100 {
        #[cfg(unix)]
        if UnixStream::connect(&socket).is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            bail!("Native workspace exited during launch: {status}");
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("Workspace launch is still pending; check its window before retrying")
}
