use crate::{
    Context, Result, bail,
    profile::{Store, regular_file},
    safe_fs,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
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

pub fn prepare(store: &Store, id: &str, cache: Option<&Path>) -> Result<()> {
    let mut profile = store.load(id)?;
    let dir = store.dir(id)?;
    let _lock = safe_fs::Lock::acquire(&dir.join("profile.lock"))?;
    if profile.ready || dir.join("disk.img").exists() {
        bail!("Profile already has a disk; it will not be overwritten");
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
    for sub in ["mailbox", "mailbox/commands", "mailbox/replies", "kit"] {
        safe_fs::private_dir(&dir.join(sub))?;
    }
    fs::write(
        dir.join("kit/worker.py"),
        include_str!("../guest/worker.py"),
    )?;
    fs::write(dir.join("kit/setup.sh"), include_str!("../guest/setup.sh"))?;
    fs::write(
        dir.join("kit/welcome.html"),
        include_str!("../guest/welcome.html"),
    )?;
    for name in ["kernel", "initrd", "disk.img", "seed.iso"] {
        fs::rename(stage.join(name), dir.join(name))?;
    }
    profile.ready = true;
    store.save(&profile)?;
    eprintln!(
        "Prepared {}. Start the VM; desktop packages install on first boot.",
        profile.name
    );
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
