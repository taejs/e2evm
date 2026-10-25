use crate::{Context, Result, bail, profile::Store, safe_fs};
use serde_json::{Value, json};
use std::{fs, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

pub fn online(store: &Store, id: &str) -> bool {
    let result = (|| -> Result<bool> {
        let directory = safe_fs::Directory::open(&store.dir(id)?.join("mailbox"))?;
        let value: Value = serde_json::from_slice(&directory.read("heartbeat.json", 4096)?)?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
        let timestamp = value["timestamp"].as_f64().context("Invalid heartbeat")?;
        Ok(timestamp <= now + 5.0 && now - timestamp < 10.0)
    })();
    result.unwrap_or(false)
}

pub fn execute(store: &Store, id: &str, argv: &[String], timeout: u64) -> Result<Value> {
    store.load(id)?;
    if argv.is_empty() || argv.len() > 256 || argv.iter().any(|arg| arg.contains('\0')) { bail!("Provide a nonempty guest argument vector"); }
    if !(1..=600).contains(&timeout) { bail!("Guest timeout must be 1–600 seconds"); }
    if !online(store,id) { bail!("Guest worker is offline. Start the VM and wait for guest readiness"); }
    let mailbox = store.dir(id)?.join("mailbox");
    for dir in [&mailbox, &mailbox.join("commands"), &mailbox.join("replies")] {
        let meta = fs::symlink_metadata(dir)?;
        if !meta.is_dir() || meta.file_type().is_symlink() { bail!("Mailbox must contain real directories"); }
    }
    let commands = safe_fs::Directory::open(&mailbox.join("commands"))?;
    let replies = safe_fs::Directory::open(&mailbox.join("replies"))?;
    let request_id = safe_fs::unique_id();
    let request = json!({"version":1,"id":request_id,"argv":argv,"timeout":timeout,"expires_at":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()+10});
    let bytes = serde_json::to_vec(&request)?;
    if bytes.len() > 65536 { bail!("Guest command is too large"); }
    let filename = format!("{request_id}.json");
    commands.write_new(&filename, &bytes)?;
    let deadline = Instant::now() + Duration::from_secs(timeout+15);
    while Instant::now() < deadline {
        match replies.read(&filename, 2*1024*1024) {
          Ok(bytes) => {
            let result: Value = serde_json::from_slice(&bytes)?;
            if result["id"] != request_id { bail!("Guest reply ID mismatch"); }
            replies.remove(&filename)?;
            return Ok(result);
          }
          Err(error) if error.downcast_ref::<std::io::Error>().is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) => {}
          Err(error) => return Err(error),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("Guest request {request_id} has an unknown outcome; it was not retried")
}
