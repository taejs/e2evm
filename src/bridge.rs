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
    request_if_running(store, id, action, payload)?
        .context("Workspace is not running; run e2evm app")
}

/// A missing or refused socket has no listening workspace. Once connected,
/// protocol failures remain errors: an uncertain operation is never retried.
pub fn request_if_running(
    store: &Store,
    id: &str,
    action: &str,
    payload: Value,
) -> Result<Option<Value>> {
    crate::profile::validate_id(id)?;
    #[cfg(unix)]
    {
        let path = store.root.join("control.sock");
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("Cannot inspect workspace control socket"),
        };
        if !metadata.file_type().is_socket()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            bail!("Unsafe workspace control socket");
        }
        let mut socket = match UnixStream::connect(path) {
            Ok(socket) => socket,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error).context("Cannot connect to workspace"),
        };
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
        Ok(Some(value))
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
    if let Some(path) = std::env::var_os("E2EVM_APP").or_else(|| std::env::var_os("HARBORVM_APP")) {
        candidates.push(PathBuf::from(path));
    }
    if let Some(parent) = current.parent() {
        for name in [
            "E2EVMDesktop",
            "E2EVM.app",
            "../../dist/E2EVM.app",
            "HarborVMDesktop",
            "HarborVM.app",
            "../../dist/HarborVM.app",
        ] {
            candidates.push(parent.join(name));
        }
    }
    for path in candidates {
        if path.extension().is_some_and(|ext| ext == "app") {
            // Renamed bundles may still contain the legacy executable. Inspect what exists.
            let names = if path.file_name().is_some_and(|name| name == "HarborVM.app") {
                ["HarborVMDesktop", "E2EVMDesktop"]
            } else {
                ["E2EVMDesktop", "HarborVMDesktop"]
            };
            for name in names {
                let executable = path.join("Contents/MacOS").join(name);
                if executable.is_file() {
                    return Ok(executable);
                }
            }
        } else if path.is_file() {
            return Ok(path);
        }
    }
    bail!("Cannot find E2EVM.app; build it with scripts/build-macos.sh or set E2EVM_APP")
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
