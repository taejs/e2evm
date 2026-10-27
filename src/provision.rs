use crate::{
    Context, Result, bail,
    profile::{Store, regular_file},
    safe_fs,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const BASE: &str = "https://cloud-images.ubuntu.com/resolute/20261016/";
const PREFIX: &str = "resolute-server-cloudimg-arm64";

fn checked(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        bail!("External preparation step failed with {status}");
    }
    Ok(())
}
fn download(url: &str, output: &Path) -> Result<()> {
    checked(
        Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--proto",
                "=https",
                "--tlsv1.2",
                "--retry",
                "2",
                url,
                "-o",
            ])
            .arg(output),
    )
}
fn digest(path: &Path) -> Result<String> {
    regular_file(path)?;
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn artifact(
    downloads: &Path,
    cache: Option<&Path>,
    name: &str,
    sums: &str,
    unpacked: bool,
) -> Result<PathBuf> {
    let expected = sums
        .lines()
        .find_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() == 2 && fields[1].trim_start_matches('*') == name {
                Some(fields[0])
            } else {
                None
            }
        })
        .context("Artifact not in official checksum manifest")?;
    if expected.len() != 64 {
        bail!("Invalid checksum manifest");
    }
    let path = if let Some(cache) = cache {
        cache.join(name)
    } else {
        downloads.join(name)
    };
    if !path.exists() {
        if cache.is_some() {
            bail!("Cached artifact missing: {}", path.display());
        }
        download(
            &format!("{BASE}{}{name}", if unpacked { "unpacked/" } else { "" }),
            &path,
        )?;
    }
    eprintln!("Verifying {name}…");
    if digest(&path)? != expected {
        bail!("SHA256 mismatch for {name}; no profile was prepared");
    }
    Ok(path)
}
fn u32_at(data: &[u8], i: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(
        data.get(i..i + 4)
            .context("Truncated kernel header")?
            .try_into()?,
    ) as usize)
}
fn kernel(source: &Path, output: &Path) -> Result<()> {
    let mut data = fs::read(source)?;
    if data.starts_with(&[0x1f, 0x8b]) {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        return checked(Command::new("gzip").args(["-dc"]).arg(source).stdout(file));
    }
    if data.get(56..60) == Some(b"ARM\x64") {
        fs::write(output, data)?;
        return Ok(());
    }
    let pe = u32_at(&data, 60)?;
    let sections = u16::from_le_bytes(
        data.get(pe + 6..pe + 8)
            .context("Invalid PE header")?
            .try_into()?,
    ) as usize;
    let opt = u16::from_le_bytes(
        data.get(pe + 20..pe + 22)
            .context("Invalid PE header")?
            .try_into()?,
    ) as usize;
    for index in 0..sections {
        let entry = pe + 24 + opt + 40 * index;
        if data.get(entry..entry + 6) == Some(b".linux") {
            let size = u32_at(&data, entry + 16)?;
            let offset = u32_at(&data, entry + 20)?;
            data = data
                .get(offset..offset + size)
                .context("Invalid kernel section")?
                .to_vec();
            break;
        }
    }
    if data.get(4..8) != Some(b"zimg") || data.get(24..29) != Some(b"zstd\0") {
        bail!("Unsupported kernel; need ARM64 Image or zstd zboot");
    }
    let offset = u32_at(&data, 8)?;
    let size = u32_at(&data, 12)?;
    let payload = data
        .get(offset..offset + size)
        .context("Invalid zboot payload")?;
    let out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut child=Command::new("node").args(["-e","process.stdout.write(require('node:zlib').zstdDecompressSync(require('node:fs').readFileSync(0)))"]).stdin(Stdio::piped()).stdout(out).spawn().context("Node.js 22.15+ is required to unpack Ubuntu zboot kernels")?;
    child
        .stdin
        .take()
        .context("Missing kernel converter input")?
        .write_all(payload)?;
    if !child.wait()?.success() {
        bail!("Kernel conversion failed");
    }
    let mut file = fs::File::open(output)?;
    let mut header = [0; 64];
    file.read_exact(&mut header)?;
    if &header[56..60] != b"ARM\x64" {
        bail!("Converted kernel is not ARM64 Image");
    }
    Ok(())
}

const LINUX_ARTIFACTS: &[&str] = &["kernel", "initrd", "disk.img", "seed.iso"];
const MACOS_ARTIFACTS: &[&str] = &[
    "disk.img",
    "hardware-model.bin",
    "machine-identifier.bin",
    "auxiliary-storage",
];
const KIT_FILES: &[&str] = &[
    "worker.py",
    "setup.sh",
    "setup-macos.sh",
    "welcome.html",
    "setup-options.env",
];
const PROMOTION_JOURNAL: &str = "promotion.json";

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PreparationSpec {
    guest_os: crate::profile::GuestOS,
    cpus: u32,
    memory_mib: u32,
    disk_gib: u32,
    setup: [bool; 5],
}
impl PreparationSpec {
    fn from_profile(profile: &crate::profile::Profile) -> Self {
        Self {
            guest_os: profile.guest_os,
            cpus: profile.cpus,
            memory_mib: profile.memory_mib,
            disk_gib: profile.disk_gib,
            setup: [
                profile.setup.desktop,
                profile.setup.browser,
                profile.setup.python,
                profile.setup.node,
                profile.setup.playwright,
            ],
        }
    }
    fn artifacts(&self) -> &'static [&'static str] {
        match self.guest_os {
            crate::profile::GuestOS::Linux => LINUX_ARTIFACTS,
            crate::profile::GuestOS::Macos => MACOS_ARTIFACTS,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ObjectIdentity {
    dev: u64,
    ino: u64,
    uid: u32,
}
impl ObjectIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            uid: metadata.uid(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ArtifactIdentity {
    object: ObjectIdentity,
    length: u64,
    modified: i64,
    modified_nanos: i64,
}
impl ArtifactIdentity {
    fn inspect(path: &Path) -> Result<Option<Self>> {
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } {
            bail!(
                "Preparation artifact must be a regular file owned by this user: {}",
                path.display()
            );
        }
        file.sync_all()?;
        Ok(Some(Self {
            object: ObjectIdentity::from_metadata(&metadata),
            length: metadata.len(),
            modified: metadata.mtime(),
            modified_nanos: metadata.mtime_nsec(),
        }))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PromotionJournal {
    version: u32,
    spec: PreparationSpec,
    profile_directory: ObjectIdentity,
    staging_directory: ObjectIdentity,
    kit_directory: ObjectIdentity,
    artifacts: Vec<ArtifactIdentity>,
    kit: Vec<ArtifactIdentity>,
}

fn preparation_directory(path: &Path) -> Result<fs::File> {
    // Reject linked ancestors and keep a directory handle for durable updates.
    let _directory = safe_fs::Directory::open(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        bail!(
            "Preparation directory must be private and owned by this user: {}",
            path.display()
        );
    }
    Ok(file)
}

fn record_promotion(
    store: &Store,
    id: &str,
    profile: &crate::profile::Profile,
) -> Result<PromotionJournal> {
    let dir = store.dir(id)?;
    let stage = dir.join("preparing");
    let profile_directory = preparation_directory(&dir)?;
    let staging_directory = preparation_directory(&stage)?;
    let kit_directory = preparation_directory(&dir.join("kit"))?;
    let spec = PreparationSpec::from_profile(profile);
    let mut artifacts = Vec::new();
    for name in spec.artifacts() {
        if fs::symlink_metadata(dir.join(name)).is_ok() {
            bail!("Preparation destination already exists; it will not be replaced: {name}");
        }
        artifacts.push(
            ArtifactIdentity::inspect(&stage.join(name))?
                .with_context(|| format!("Prepared artifact is missing: {name}"))?,
        );
    }
    let mut kit = Vec::new();
    for name in KIT_FILES {
        kit.push(
            ArtifactIdentity::inspect(&dir.join("kit").join(name))?
                .with_context(|| format!("Prepared guest kit is missing: {name}"))?,
        );
    }
    for relative in ["mailbox/commands", "mailbox/replies", "mailbox"] {
        preparation_directory(&dir.join(relative))?.sync_all()?;
    }
    staging_directory.sync_all()?;
    kit_directory.sync_all()?;
    profile_directory.sync_all()?;
    let journal = PromotionJournal {
        version: 1,
        spec,
        profile_directory: ObjectIdentity::from_metadata(&profile_directory.metadata()?),
        staging_directory: ObjectIdentity::from_metadata(&staging_directory.metadata()?),
        kit_directory: ObjectIdentity::from_metadata(&kit_directory.metadata()?),
        artifacts,
        kit,
    };
    // Publish atomically without replacing another journal, only after every prepared file is durable.
    safe_fs::Directory::open(&stage)?
        .write_new(PROMOTION_JOURNAL, &serde_json::to_vec(&journal)?)?;
    staging_directory.sync_all()?;
    Ok(journal)
}

fn resume_promotion(store: &Store, id: &str) -> Result<bool> {
    let stage = store.dir(id)?.join("preparing");
    match fs::symlink_metadata(stage.join(PROMOTION_JOURNAL)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    preparation_directory(&stage)?;
    ArtifactIdentity::inspect(&stage.join(PROMOTION_JOURNAL))?
        .context("Preparation journal disappeared")?;
    let journal: PromotionJournal =
        serde_json::from_slice(&safe_fs::read(&stage.join(PROMOTION_JOURNAL), 65536)?)?;
    promote_prepared(store, id, &journal, |_| Ok(()))?;
    eprintln!("Recovered completed preparation for {id}; the guest is ready to start.");
    Ok(true)
}

#[derive(Clone, Copy, Debug)]
enum PromotionBoundary {
    Linked,
    Unlinked,
    BeforeReady,
}

fn promote_prepared(
    store: &Store,
    id: &str,
    journal: &PromotionJournal,
    mut boundary: impl FnMut(PromotionBoundary) -> Result<()>,
) -> Result<()> {
    let dir = store.dir(id)?;
    let stage = dir.join("preparing");
    let profile_directory = preparation_directory(&dir)?;
    let staging_directory = preparation_directory(&stage)?;
    let kit_directory = preparation_directory(&dir.join("kit"))?;
    if journal.version != 1
        || journal.spec != PreparationSpec::from_profile(&store.load(id)?)
        || journal.profile_directory
            != ObjectIdentity::from_metadata(&profile_directory.metadata()?)
        || journal.staging_directory
            != ObjectIdentity::from_metadata(&staging_directory.metadata()?)
        || journal.kit_directory != ObjectIdentity::from_metadata(&kit_directory.metadata()?)
        || journal.artifacts.len() != journal.spec.artifacts().len()
        || journal.kit.len() != KIT_FILES.len()
    {
        bail!("Preparation journal does not match this profile; retained files were not changed");
    }
    for (name, identity) in KIT_FILES.iter().zip(&journal.kit) {
        if ArtifactIdentity::inspect(&dir.join("kit").join(name))?.as_ref() != Some(identity) {
            bail!("Prepared guest kit changed: {name}; retained files were not changed");
        }
    }
    // Validate the entire set before the first mutation, including later destination conflicts.
    for (name, identity) in journal.spec.artifacts().iter().zip(&journal.artifacts) {
        let source = ArtifactIdentity::inspect(&stage.join(name))?;
        let target = ArtifactIdentity::inspect(&dir.join(name))?;
        if (source.is_none() && target.is_none())
            || source.as_ref().is_some_and(|value| value != identity)
            || target.as_ref().is_some_and(|value| value != identity)
        {
            bail!("Prepared artifact identity conflict: {name}; no disk will be overwritten");
        }
    }
    for relative in ["mailbox/commands", "mailbox/replies", "mailbox"] {
        preparation_directory(&dir.join(relative))?.sync_all()?;
    }
    for (name, identity) in journal.spec.artifacts().iter().zip(&journal.artifacts) {
        let source = stage.join(name);
        let target = dir.join(name);
        if ArtifactIdentity::inspect(&target)?.is_none() {
            // hard_link is atomic and never replaces a destination. A crash can leave both names;
            // their recorded inode identity makes that state safe to finish on the next attempt.
            fs::hard_link(&source, &target)?;
            if ArtifactIdentity::inspect(&target)?.as_ref() != Some(identity) {
                bail!("Prepared artifact changed during promotion: {name}");
            }
            profile_directory.sync_all()?;
            boundary(PromotionBoundary::Linked)?;
        }
        if let Some(current) = ArtifactIdentity::inspect(&source)? {
            if &current != identity {
                bail!("Staged artifact changed during promotion: {name}");
            }
            fs::remove_file(&source)?;
            staging_directory.sync_all()?;
            boundary(PromotionBoundary::Unlinked)?;
        }
    }
    for (name, identity) in journal.spec.artifacts().iter().zip(&journal.artifacts) {
        if ArtifactIdentity::inspect(&dir.join(name))?.as_ref() != Some(identity) {
            bail!("Promoted artifact changed before readiness: {name}");
        }
    }
    profile_directory.sync_all()?;
    boundary(PromotionBoundary::BeforeReady)?;
    // Merge only readiness; assistant settings can change while a large install is in progress.
    let mut matches = false;
    store.update(id, |profile| {
        matches = journal.spec == PreparationSpec::from_profile(profile);
        if matches {
            profile.ready = true;
        }
    })?;
    if !matches {
        bail!("Profile preparation settings changed; completed files remain recoverable");
    }
    profile_directory.sync_all()?;
    // A failed metadata write or lease acquisition retains the journal for the next prepare call.
    fs::remove_file(stage.join(PROMOTION_JOURNAL))?;
    staging_directory.sync_all()?;
    Ok(())
}

pub fn prepare(store: &Store, id: &str, cache: Option<&Path>) -> Result<()> {
    let profile = store.load(id)?;
    if profile.guest_os != crate::profile::GuestOS::Linux {
        bail!("Ubuntu preparation requires a Linux profile");
    }
    let dir = store.dir(id)?;
    let _lock = safe_fs::Lock::acquire(&dir.join("profile.lock"))?;
    if profile.ready {
        bail!("Profile is already prepared; its disk will not be overwritten");
    }
    if resume_promotion(store, id)? {
        return Ok(());
    }
    if fs::symlink_metadata(dir.join("disk.img")).is_ok() {
        bail!("Profile already has a disk without a recovery journal; it will not be overwritten");
    }
    // Keep failed staging for inspection, never promote an incomplete machine.
    let stage = dir.join("preparing");
    fs::create_dir(&stage).context("Preparation staging exists; inspect it before retrying")?;
    safe_fs::private_dir(&stage)?;
    let downloads = stage.join("downloads");
    safe_fs::private_dir(&downloads)?;
    download(&format!("{BASE}SHA256SUMS"), &downloads.join("SHA256SUMS"))?;
    download(
        &format!("{BASE}unpacked/SHA256SUMS"),
        &downloads.join("UNPACKED-SHA256SUMS"),
    )?;
    let sums = fs::read_to_string(downloads.join("SHA256SUMS"))?;
    let unpacked = fs::read_to_string(downloads.join("UNPACKED-SHA256SUMS"))?;
    let archive = artifact(&downloads, cache, &format!("{PREFIX}.tar.gz"), &sums, false)?;
    let vmlinuz = artifact(
        &downloads,
        cache,
        &format!("{PREFIX}-vmlinuz-generic"),
        &unpacked,
        true,
    )?;
    let initrd = artifact(
        &downloads,
        cache,
        &format!("{PREFIX}-initrd-generic"),
        &unpacked,
        true,
    )?;
    kernel(&vmlinuz, &stage.join("kernel"))?;
    fs::copy(initrd, stage.join("initrd"))?;
    let listing = Command::new("tar").arg("-tzf").arg(&archive).output()?;
    if !listing.status.success() {
        bail!("Cannot list verified Ubuntu archive");
    }
    let listing = String::from_utf8(listing.stdout)?;
    let images: Vec<_> = listing
        .lines()
        .filter(|name| {
            name.ends_with(".img") && !name.starts_with('/') && !name.split('/').any(|p| p == "..")
        })
        .collect();
    if images.len() != 1 {
        bail!("Expected one raw image in archive");
    }
    let disk = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(stage.join("disk.img"))?;
    checked(
        Command::new("tar")
            .arg("-xOzf")
            .arg(&archive)
            .arg(images[0])
            .stdout(disk.try_clone()?),
    )?;
    disk.set_len(u64::from(profile.disk_gib) * 1024 * 1024 * 1024)?;
    let seed = stage.join("seed");
    safe_fs::private_dir(&seed)?;
    fs::write(seed.join("user-data"), cloud_config(id))?;
    fs::write(
        seed.join("meta-data"),
        format!(
            "instance-id: harborvm-{}\nlocal-hostname: {id}\n",
            safe_fs::unique_id()
        ),
    )?;
    checked(
        Command::new("hdiutil")
            .args([
                "makehybrid",
                "-iso",
                "-joliet",
                "-default-volume-name",
                "cidata",
                "-o",
            ])
            .arg(stage.join("seed.iso"))
            .arg(&seed)
            .stdout(Stdio::null()),
    )?;
    prepare_kit(store, id, &profile.setup)?;
    let journal = record_promotion(store, id, &profile)?;
    promote_prepared(store, id, &journal, |_| Ok(()))?;
    eprintln!(
        "Prepared {}. Start the VM; selected components install on first boot.",
        profile.name
    );
    Ok(())
}

fn prepare_kit(store: &Store, id: &str, setup: &crate::profile::SetupOptions) -> Result<()> {
    let dir = store.dir(id)?;
    for sub in ["mailbox", "mailbox/commands", "mailbox/replies", "kit"] {
        safe_fs::private_dir(&dir.join(sub))?;
    }
    fs::write(
        dir.join("kit/worker.py"),
        include_str!("../guest/worker.py"),
    )?;
    fs::write(dir.join("kit/setup.sh"), include_str!("../guest/setup.sh"))?;
    fs::write(
        dir.join("kit/setup-macos.sh"),
        include_str!("../guest/setup-macos.sh"),
    )?;
    fs::write(
        dir.join("kit/welcome.html"),
        include_str!("../guest/welcome.html"),
    )?;
    fs::write(
        dir.join("kit/setup-options.env"),
        format!(
            "SETUP_DESKTOP={}\nSETUP_BROWSER={}\nSETUP_PYTHON={}\nSETUP_NODE={}\nSETUP_PLAYWRIGHT={}\n",
            u8::from(setup.desktop),
            u8::from(setup.browser),
            u8::from(setup.python),
            u8::from(setup.node),
            u8::from(setup.playwright)
        ),
    )?;
    Ok(())
}

pub fn prepare_macos(store: &Store, id: &str, restore_image: Option<&Path>) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("macOS guests require Apple Silicon running macOS");
    }
    let profile = store.load(id)?;
    if profile.guest_os != crate::profile::GuestOS::Macos {
        bail!("Expected a macOS profile");
    }
    let dir = store.dir(id)?;
    let _lock = safe_fs::Lock::acquire(&dir.join("profile.lock"))?;
    if profile.ready {
        bail!("Profile is already prepared; its disk will not be overwritten");
    }
    if resume_promotion(store, id)? {
        return Ok(());
    }
    if fs::symlink_metadata(dir.join("disk.img")).is_ok() {
        bail!("Profile already has a disk without a recovery journal; it will not be overwritten");
    }
    if let Some(image) = restore_image {
        regular_file(image)?;
    }
    let stage = dir.join("preparing");
    fs::create_dir(&stage).context("Preparation staging exists; inspect it before retrying")?;
    safe_fs::private_dir(&stage)?;
    let mut command = Command::new(crate::bridge::executable()?);
    command
        .arg("--prepare-macos")
        .arg(&stage)
        .arg("--cpus")
        .arg(profile.cpus.to_string())
        .arg("--memory-mib")
        .arg(profile.memory_mib.to_string())
        .arg("--disk-gib")
        .arg(profile.disk_gib.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    if let Some(image) = restore_image {
        command.arg("--restore-image").arg(image);
    }
    checked(&mut command)?;
    prepare_kit(store, id, &profile.setup)?;
    let journal = record_promotion(store, id, &profile)?;
    promote_prepared(store, id, &journal, |_| Ok(()))?;
    eprintln!("Installed macOS. Start the guest and complete Apple's setup inside its window.");
    Ok(())
}
fn cloud_config(id: &str) -> String {
    format!(
        r#"#cloud-config
hostname: {id}
manage_etc_hosts: true
users:
  - name: harbor
    groups: [adm, sudo, video, audio]
    shell: /bin/bash
    lock_passwd: true
    sudo: ALL=(ALL) NOPASSWD:ALL
ssh_pwauth: false
disable_root: true
growpart:
  mode: auto
  devices: ['/']
resize_rootfs: true
mounts:
  - [HarborVM, /mnt/harbor, virtiofs, 'defaults,nofail', '0', '0']
  - [HarborKit, /mnt/harbor-kit, virtiofs, 'ro,defaults,nofail', '0', '0']
runcmd:
  - [sh, -c, 'sh /mnt/harbor-kit/setup.sh > /mnt/harbor/bootstrap.log 2>&1']
"#
    )
}

#[cfg(test)]
mod promotion_tests {
    use super::*;
    use crate::profile::{GuestOS, Profile};

    fn fixture(os: GuestOS) -> (tempfile::TempDir, Store, PromotionJournal) {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("runtime")).unwrap();
        let mut profile = Profile::new("test", "Test", 2, 4096).unwrap();
        profile.guest_os = os;
        profile.disk_gib = 64;
        store.create(&profile).unwrap();
        let dir = store.dir("test").unwrap();
        safe_fs::private_dir(&dir.join("preparing")).unwrap();
        for name in PreparationSpec::from_profile(&profile).artifacts() {
            fs::write(dir.join("preparing").join(name), format!("prepared-{name}")).unwrap();
        }
        prepare_kit(&store, "test", &profile.setup).unwrap();
        let journal = record_promotion(&store, "test", &profile).unwrap();
        (temp, store, journal)
    }

    #[test]
    fn prepare_resumes_a_completed_install_after_disk_promotion() {
        let (_temp, store, _) = fixture(GuestOS::Linux);
        let dir = store.dir("test").unwrap();
        for name in ["kernel", "initrd", "disk.img"] {
            fs::rename(dir.join("preparing").join(name), dir.join(name)).unwrap();
        }
        prepare(&store, "test", None).unwrap();
        assert!(store.load("test").unwrap().ready);
    }

    #[test]
    fn every_promotion_boundary_recovers_for_both_operating_systems() {
        for os in [GuestOS::Linux, GuestOS::Macos] {
            for fail_at in 1..=9 {
                let (_temp, store, journal) = fixture(os);
                let mut seen = 0;
                let failure = promote_prepared(&store, "test", &journal, |_| {
                    seen += 1;
                    if seen == fail_at {
                        bail!("injected interruption")
                    }
                    Ok(())
                })
                .unwrap_err();
                assert!(failure.to_string().contains("injected interruption"));
                assert!(!store.load("test").unwrap().ready);
                assert!(resume_promotion(&store, "test").unwrap());
                assert!(store.load("test").unwrap().ready);
                let dir = store.dir("test").unwrap();
                for (name, identity) in journal.spec.artifacts().iter().zip(&journal.artifacts) {
                    assert_eq!(
                        ArtifactIdentity::inspect(&dir.join(name)).unwrap().as_ref(),
                        Some(identity)
                    );
                    assert!(!dir.join("preparing").join(name).exists());
                }
                assert!(!dir.join("preparing").join(PROMOTION_JOURNAL).exists());
            }
        }
    }

    #[test]
    fn metadata_lease_conflict_preserves_a_resumable_install_and_new_settings() {
        let (_temp, store, journal) = fixture(GuestOS::Linux);
        let dir = store.dir("test").unwrap();
        let lease = safe_fs::Lock::acquire(&dir.join("metadata.lock")).unwrap();
        assert!(promote_prepared(&store, "test", &journal, |_| Ok(())).is_err());
        assert!(!store.load("test").unwrap().ready);
        assert!(dir.join("preparing").join(PROMOTION_JOURNAL).exists());
        drop(lease);
        store
            .update("test", |profile| {
                profile.assistant_program = Some("new-codex".into())
            })
            .unwrap();
        assert!(resume_promotion(&store, "test").unwrap());
        let profile = store.load("test").unwrap();
        assert!(profile.ready);
        assert_eq!(profile.assistant_program.as_deref(), Some("new-codex"));
    }

    #[test]
    fn foreign_destination_and_symlinks_are_never_overwritten() {
        for symlink in [false, true] {
            let (_temp, store, journal) = fixture(GuestOS::Linux);
            let dir = store.dir("test").unwrap();
            let target = dir.join("seed.iso"); // Last artifact: preflight must catch it before moving anything.
            let unrelated = dir.join("unrelated.img");
            fs::write(&unrelated, b"unrelated").unwrap();
            if symlink {
                std::os::unix::fs::symlink(&unrelated, &target).unwrap();
            } else {
                fs::write(&target, b"foreign destination").unwrap();
            }
            assert!(promote_prepared(&store, "test", &journal, |_| Ok(())).is_err());
            assert!(!store.load("test").unwrap().ready);
            assert!(!dir.join("disk.img").exists());
            assert!(dir.join("preparing/disk.img").exists());
            assert_eq!(fs::read(&unrelated).unwrap(), b"unrelated");
            if symlink {
                assert!(
                    fs::symlink_metadata(&target)
                        .unwrap()
                        .file_type()
                        .is_symlink()
                );
            } else {
                assert_eq!(fs::read(&target).unwrap(), b"foreign destination");
            }
        }
    }

    #[test]
    fn changed_source_kit_or_spec_does_not_become_ready() {
        for change in ["source", "kit", "spec", "missing", "source-symlink"] {
            let (_temp, store, journal) = fixture(GuestOS::Linux);
            let dir = store.dir("test").unwrap();
            match change {
                "source" => {
                    // Same bytes and length in a different inode must still be rejected.
                    let source = dir.join("preparing/disk.img");
                    fs::rename(&source, dir.join("original-disk")).unwrap();
                    fs::write(&source, b"prepared-disk.img").unwrap();
                }
                "kit" => fs::write(dir.join("kit/worker.py"), b"changed").unwrap(),
                "spec" => {
                    store.update("test", |profile| profile.cpus = 4).unwrap();
                }
                "missing" => fs::remove_file(dir.join("preparing/disk.img")).unwrap(),
                "source-symlink" => {
                    let source = dir.join("preparing/disk.img");
                    fs::rename(&source, dir.join("original-disk")).unwrap();
                    std::os::unix::fs::symlink(dir.join("original-disk"), &source).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                promote_prepared(&store, "test", &journal, |_| Ok(())).is_err(),
                "{change}"
            );
            assert!(!store.load("test").unwrap().ready);
            assert!(!dir.join("disk.img").exists());
        }
    }

    #[test]
    fn incomplete_preparation_without_journal_is_retained() {
        let (_temp, store, _) = fixture(GuestOS::Linux);
        let dir = store.dir("test").unwrap();
        fs::remove_file(dir.join("preparing").join(PROMOTION_JOURNAL)).unwrap();
        assert!(!resume_promotion(&store, "test").unwrap());
        assert!(prepare(&store, "test", None).is_err());
        assert!(dir.join("preparing/disk.img").exists());
        assert!(!dir.join("disk.img").exists());
        assert!(!store.load("test").unwrap().ready);
    }

    #[test]
    fn no_journal_is_published_for_an_incomplete_artifact_set() {
        let (_temp, store, _) = fixture(GuestOS::Linux);
        let dir = store.dir("test").unwrap();
        fs::remove_file(dir.join("preparing").join(PROMOTION_JOURNAL)).unwrap();
        fs::remove_file(dir.join("preparing/seed.iso")).unwrap();
        assert!(record_promotion(&store, "test", &store.load("test").unwrap()).is_err());
        assert!(!dir.join("preparing").join(PROMOTION_JOURNAL).exists());
        assert!(dir.join("preparing/disk.img").exists());
    }

    #[test]
    fn complete_journal_resumes_without_reinstalling_or_original_restore_image() {
        for os in [GuestOS::Linux, GuestOS::Macos] {
            let (_temp, store, _) = fixture(os);
            match os {
                GuestOS::Linux => {
                    prepare(&store, "test", Some(Path::new("/no-longer-present-cache"))).unwrap()
                }
                GuestOS::Macos => {
                    #[cfg(target_os = "macos")]
                    prepare_macos(&store, "test", Some(Path::new("/no-longer-present.ipsw")))
                        .unwrap();
                    #[cfg(not(target_os = "macos"))]
                    assert!(resume_promotion(&store, "test").unwrap());
                }
            }
            assert!(store.load("test").unwrap().ready);
        }
    }

    #[test]
    fn malformed_journal_and_replaced_staging_directory_are_rejected() {
        for replace_directory in [false, true] {
            let (_temp, store, _) = fixture(GuestOS::Linux);
            let dir = store.dir("test").unwrap();
            let stage = dir.join("preparing");
            if replace_directory {
                fs::rename(&stage, dir.join("original-stage")).unwrap();
                safe_fs::private_dir(&stage).unwrap();
                fs::copy(
                    dir.join("original-stage").join(PROMOTION_JOURNAL),
                    stage.join(PROMOTION_JOURNAL),
                )
                .unwrap();
            } else {
                fs::write(stage.join(PROMOTION_JOURNAL), b"not JSON").unwrap();
            }
            assert!(resume_promotion(&store, "test").is_err());
            assert!(!store.load("test").unwrap().ready);
            assert!(!dir.join("disk.img").exists());
        }
    }
}
