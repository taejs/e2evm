use crate::{Context, Result, bail, safe_fs};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub cpus: u32,
    pub memory_mib: u32,
    pub disk_gib: u32,
    pub ready: bool,
    #[serde(default)]
    pub assistant_program: Option<String>,
}
impl Profile {
    pub fn new(id: &str, name: &str, cpus: u32, memory_mib: u32) -> Result<Self> {
        let profile = Self {
            version: 1,
            id: id.into(),
            name: name.into(),
            cpus,
            memory_mib,
            disk_gib: 24,
            ready: false,
            assistant_program: None,
        };
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<()> {
        validate_id(&self.id)?;
        if self.version != 1 {
            bail!("Unsupported profile version");
        }
        if self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(char::is_control)
        {
            bail!("Name must contain 1–80 printable bytes");
        }
        if !(1..=32).contains(&self.cpus) {
            bail!("CPU count must be 1–32");
        }
        if !(512..=65536).contains(&self.memory_mib) {
            bail!("Memory must be 512–65536 MiB");
        }
        if !(8..=512).contains(&self.disk_gib) {
            bail!("Disk size must be 8–512 GiB");
        }
        Ok(())
    }
}
pub fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 40
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || id.starts_with('-')
    {
        bail!(
            "Profile ID must be 1–40 lowercase letters, digits or hyphens, starting with a letter or digit"
        );
    }
    Ok(())
}
#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        safe_fs::private_dir(&root)?;
        let root = root.canonicalize()?;
        safe_fs::private_dir(&root.join("profiles"))?;
        Ok(Self { root })
    }
    pub fn default_root() -> Result<PathBuf> {
        if let Some(value) = std::env::var_os("HARBORVM_HOME") {
            return Ok(value.into());
        }
        let home = std::env::var_os("HOME").context("HOME is not set; provide HARBORVM_HOME")?;
        Ok(PathBuf::from(home).join("Library/Application Support/HarborVM"))
    }
    pub fn dir(&self, id: &str) -> Result<PathBuf> {
        validate_id(id)?;
        let path = self.root.join("profiles").join(id);
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.is_dir() || meta.file_type().is_symlink() {
                bail!("Profile directory must not be a link");
            }
        }
        Ok(path)
    }
    pub fn load(&self, id: &str) -> Result<Profile> {
        let profile: Profile =
            serde_json::from_slice(&safe_fs::read(&self.dir(id)?.join("profile.json"), 65536)?)?;
        profile.validate()?;
        if profile.id != id {
            bail!("Profile ID does not match its directory");
        }
        Ok(profile)
    }
    pub fn create(&self, profile: &Profile) -> Result<()> {
        profile.validate()?;
        let dir = self.dir(&profile.id)?;
        fs::create_dir(&dir).context("Profile already exists or cannot be created")?;
        safe_fs::private_dir(&dir)?;
        self.save(profile)
    }
    pub fn save(&self, profile: &Profile) -> Result<()> {
        profile.validate()?;
        safe_fs::atomic_write(
            &self.dir(&profile.id)?.join("profile.json"),
            &serde_json::to_vec_pretty(profile)?,
        )
    }
    pub fn list(&self) -> Result<Vec<Profile>> {
        let mut profiles = Vec::new();
        for entry in fs::read_dir(self.root.join("profiles"))? {
            let entry = entry?;
            let id = entry.file_name().to_string_lossy().into_owned();
            if !entry.file_type()?.is_dir() {
                continue;
            }
            profiles.push(self.load(&id)?);
        }
        profiles.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(profiles)
    }
    pub fn descriptor(&self, id: &str) -> Result<serde_json::Value> {
        let mut value = serde_json::to_value(self.load(id)?)?;
        value["directory"] = self.dir(id)?.to_string_lossy().into_owned().into();
        Ok(value)
    }
}
pub fn regular_file(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        bail!("Expected regular file: {}", path.display());
    }
    Ok(())
}
