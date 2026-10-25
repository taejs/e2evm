use crate::{Context, Result, bail};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::Path;

pub fn private_dir(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if !meta.is_dir() || meta.file_type().is_symlink() { bail!("Not a real directory: {}", path.display()); }
    } else {
        fs::create_dir_all(path)?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)
        .with_context(|| format!("Cannot read {}", path.display()))?;
    if !file.metadata()?.is_file() { bail!("Not a regular file: {}", path.display()); }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit { bail!("File exceeds size limit: {}", path.display()); }
    Ok(bytes)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("tmp-{}-{}", std::process::id(), unique_id()));
    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(temp); }
    result
}

pub fn unique_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    format!("{:x}-{:x}-{:x}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos(), SEQUENCE.fetch_add(1, Ordering::Relaxed))
}

pub struct Lock(File);
impl Lock {
    pub fn acquire(path: &Path) -> Result<Self> {
        let file = OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)?;
        if !file.metadata()?.is_file() { bail!("Lock is not a regular file"); }
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            bail!("Resource is already owned by another process: {}", path.display());
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) { unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN); } }
}

/// Pins a guest-writable directory so renamed ancestors cannot redirect host IO.
pub struct Directory(File);
impl Directory {
    pub fn open(path: &Path) -> Result<Self> {
        use std::path::Component;
        if !path.is_absolute() { bail!("Directory path must be absolute"); }
        let mut fd = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY).open("/")?;
        for component in path.components() {
            match component {
                Component::RootDir => continue,
                Component::Normal(name) => {
                    use std::os::unix::{ffi::OsStrExt, io::FromRawFd};
                    let name = std::ffi::CString::new(name.as_bytes())?;
                    let next = unsafe { libc::openat(fd.as_raw_fd(), name.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
                    if next < 0 { return Err(std::io::Error::last_os_error().into()); }
                    fd = unsafe { File::from_raw_fd(next) };
                }
                _ => bail!("Unsafe directory component"),
            }
        }
        Ok(Self(fd))
    }
    fn name(name: &str) -> Result<std::ffi::CString> {
        if name.is_empty() || name.contains('/') || name == "." || name == ".." { bail!("Invalid filename"); }
        Ok(std::ffi::CString::new(name)?)
    }
    pub fn read(&self, name: &str, limit: u64) -> Result<Vec<u8>> {
        use std::os::unix::io::FromRawFd;
        let name = Self::name(name)?;
        let fd = unsafe { libc::openat(self.0.as_raw_fd(), name.as_ptr(), libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC) };
        if fd < 0 {return Err(std::io::Error::last_os_error().into());}
        let file = unsafe {File::from_raw_fd(fd)};
        if !file.metadata()?.is_file() {bail!("Not a regular file");}
        let mut bytes=Vec::new(); file.take(limit+1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {bail!("File exceeds limit");} Ok(bytes)
    }
    pub fn write_new(&self, name: &str, bytes: &[u8]) -> Result<()> {
        use std::os::unix::io::FromRawFd;
        let temporary=Self::name(&format!("{}.tmp",unique_id()))?; let target=Self::name(name)?;
        let fd=unsafe {libc::openat(self.0.as_raw_fd(),temporary.as_ptr(),libc::O_WRONLY|libc::O_CREAT|libc::O_EXCL|libc::O_NOFOLLOW|libc::O_CLOEXEC,0o600)};
        if fd<0 {return Err(std::io::Error::last_os_error().into());}
        let result=(|| { let mut file=unsafe {File::from_raw_fd(fd)}; file.write_all(bytes)?; file.sync_all()?;
            // linkat publishes without replacing an existing request.
            if unsafe {libc::linkat(self.0.as_raw_fd(),temporary.as_ptr(),self.0.as_raw_fd(),target.as_ptr(),0)} !=0 {return Err(std::io::Error::last_os_error().into());} Ok(()) })();
        unsafe {libc::unlinkat(self.0.as_raw_fd(),temporary.as_ptr(),0);}
        result
    }
    pub fn remove(&self, name:&str)->Result<()> {
        let name=Self::name(name)?;
        if unsafe {libc::unlinkat(self.0.as_raw_fd(),name.as_ptr(),0)}!=0 {return Err(std::io::Error::last_os_error().into());} Ok(())
    }
}
