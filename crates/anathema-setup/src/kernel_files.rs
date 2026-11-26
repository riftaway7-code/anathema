//! The kernel files anathema-xboot boots from: `xkernel` (the macOS 27.2.1 x86_64 kernel) and
//! `xkc.bin` (the linked kext collection). Both are made on the user's machine from Apple's
//! Kernel Debug Kits, downloaded from dortania's GitHub releases; nothing Apple-made ships
//! with setup.

use crate::fat32::FileSpec;
use crate::{XCP, XCP2, XCP3};
use anathema_kc::macho::{x86_64_slice, Macho};
use anathema_kc::{link, Config};
use anathema_media::download::download_verified;
use anathema_media::hash::sha256;
use anathema_media::kdk::extract::{extract, MARKER};
use anathema_media::kdk::pin::{self, Kdk, KDK_27, KDK_TAHOE, KERNEL_PATH, KERNEL_SHA256, KERNEL_UUID};
use std::path::{Path, PathBuf};

pub struct KernelFiles {
    pub xkernel: Vec<u8>,
    pub xkc: Vec<u8>,
}

/// Written into an unpacked KDK folder: the sha256 of the dmg it came from.
pub const PIN_FILE: &str = ".anathema-pin";
/// About what one KDK's kexts and kernel take once unpacked (1.01 and 1.06 GB measured).
const UNPACKED_BYTES: u64 = 1_100_000_000;
/// Roughly what is downloaded, for the review page.
pub const DOWNLOAD_BYTES: u64 = KDK_TAHOE.size + KDK_27.size;

const ROOTS: [&str; 9] = [
    "com.apple.driver.AppleACPIPlatform",
    "com.apple.kec.corecrypto",
    "com.apple.kec.AppleEncryptedArchive",
    "com.apple.kec.XrtHostedXnu",
    "com.apple.kec.Libm",
    "com.apple.kec.Compression",
    "com.apple.kec.pthread",
    "com.apple.security.AppleImage4",
    "com.apple.kext.CoreTrust",
];
/// The extra roots the collaborator boots with; `mkkc.sh $(cat tools/kc-roots.txt)`.
const KC_ROOTS: &str = include_str!("../../../tools/kc-roots.txt");
const PREFER_LAST: [&str; 7] = [
    "com.apple.kec.corecrypto",
    "com.apple.kec.Compression",
    "com.apple.kec.pthread",
    "com.apple.kec.Libm",
    "com.apple.kec.AppleEncryptedArchive",
    "com.apple.kec.XrtHostedXnu",
    "com.apple.security.AppleImage4",
];

/// The tools/mkkc.sh link: Tahoe kexts first, the 27 KDK preferred for the kexts that must
/// match the 27 kernel.
pub fn kc_config(tahoe_ext: &Path, k27_ext: &Path, kernel: &Path) -> Config {
    let s = |v: &str| v.to_string();
    let mut roots: Vec<String> = ROOTS.iter().map(|r| s(r)).collect();
    roots.extend(KC_ROOTS.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(s));
    Config {
        kernel: kernel.to_path_buf(),
        ext: vec![tahoe_ext.to_path_buf(), k27_ext.to_path_buf()],
        base: 0xffff_ff80_0200_0000,
        roots,
        prefer_last: PREFER_LAST.iter().map(|r| s(r)).collect(),
        strip: vec![
            (s("com.apple.security.AppleImage4"), s("AppleSecurityExtension")),
            (s("com.apple.kext.CoreTrust"), s("AppleSecurityExtension")),
        ],
        codeless: vec![s("com.apple.driver.AppleMobileFileIntegrity")],
        ..Default::default()
    }
}

/// Everything that decides the output: the pins, the link options, the linker's source and this
/// version.
pub fn stamp_inputs() -> String {
    let mut text = String::new();
    for k in pin::ALL {
        text.push_str(&format!("{} {}\n", k.build, k.sha256));
    }
    text.push_str(&format!("kernel {KERNEL_SHA256}\n"));
    text.push_str(&format!("{:?}\n", kc_config(Path::new("tahoe"), Path::new("k27"), Path::new("kernel"))));
    text.push_str(&format!("linker-source {}\n", env!("ANATHEMA_LINKER_SOURCE")));
    text.push_str(env!("CARGO_PKG_VERSION"));
    text
}

pub fn stamp() -> String {
    sha256(stamp_inputs().as_bytes())
}

/// An unpacked KDK folder is reused only when it is complete and came from the pinned dmg.
fn tree_done(tree: &Path, k: &Kdk) -> bool {
    tree.join(MARKER).exists() && std::fs::read_to_string(tree.join(PIN_FILE)).is_ok_and(|s| s.trim() == k.sha256)
}

/// Free space still needed to finish: each missing KDK is downloaded (less what is already
/// on disk), unpacked, and its dmg deleted before the next one, plus a margin for the outputs.
pub fn space_needed(cache: &Path) -> u64 {
    const MARGIN: i128 = 200_000_000;
    let (mut used, mut peak) = (0i128, 0i128);
    for k in [KDK_TAHOE, KDK_27] {
        if tree_done(&cache.join(k.build), &k) {
            continue;
        }
        let dmg = cache.join(format!("{}.dmg", k.build));
        let part = cache.join(format!("{}.dmg.part", k.build));
        let have = [dmg, part].iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).max().unwrap_or(0).min(k.size);
        used += (k.size - have) as i128 + UNPACKED_BYTES as i128;
        peak = peak.max(used);
        used -= k.size as i128;
    }
    (peak + MARGIN) as u64
}

pub fn space_check(free: u64, need: u64, cache: &Path) -> Result<(), String> {
    if free >= need {
        return Ok(());
    }
    Err(format!(
        "Not enough free space for the kernel files: {:.1} GB needed, {:.1} GB free on {}",
        need as f64 / 1e9,
        free as f64 / 1e9,
        cache.display()
    ))
}

/// `%LOCALAPPDATA%\Anathema\kdk`.
pub fn default_cache() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?;
    Ok(PathBuf::from(base).join("Anathema").join("kdk"))
}

/// Records the stamp and the hashes of the two outputs; written after them, so a stamp means
/// both files are complete.
pub fn write_stamp(out: &Path, stamp: &str, xkernel: &[u8], xkc: &[u8]) -> Result<(), String> {
    let text = format!("{stamp}\nxkernel {}\nxkc {}\n", sha256(xkernel), sha256(xkc));
    let tmp = out.join("stamp.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, out.join("stamp")).map_err(|e| format!("cannot write the stamp: {e}"))
}

fn read_cache(out: &Path) -> Option<KernelFiles> {
    let text = std::fs::read_to_string(out.join("stamp")).ok()?;
    let mut lines = text.lines();
    if lines.next()? != stamp() {
        return None;
    }
    let want_kernel = lines.next()?.strip_prefix("xkernel ")?.to_string();
    let want_kc = lines.next()?.strip_prefix("xkc ")?.to_string();
    let xkernel = std::fs::read(out.join("xkernel")).ok()?;
    let xkc = std::fs::read(out.join("xkc.bin")).ok()?;
    (sha256(&xkernel) == want_kernel && sha256(&xkc) == want_kc).then_some(KernelFiles { xkernel, xkc })
}

type Fetch<'a> = dyn FnMut(&Kdk, &Path, &mut dyn FnMut(u64, u64)) -> Result<(), String> + 'a;

/// Makes (or reuses) the kernel files in `cache`, downloading through `fetch`. `free_bytes` is
/// the free space on the cache's drive. `progress` gets 0.0 to 1.0.
pub fn prepare_with(
    cache: &Path,
    free_bytes: u64,
    fetch: &mut Fetch,
    progress: &mut dyn FnMut(f32, &str),
    log: &mut dyn FnMut(String),
) -> Result<KernelFiles, String> {
    let out = cache.join("out");
    if let Some(k) = read_cache(&out) {
        log(format!("Kernel files already prepared in {}", out.display()));
        progress(1.0, "Kernel files ready");
        return Ok(k);
    }
    let kdks = [KDK_TAHOE, KDK_27];
    if kdks.iter().any(|k| !tree_done(&cache.join(k.build), k)) {
        space_check(free_bytes, space_needed(cache), cache)?;
    }
    std::fs::create_dir_all(cache).map_err(|e| format!("cannot create {}: {e}", cache.display()))?;
    for (i, k) in kdks.iter().enumerate() {
        let tree = cache.join(k.build);
        let (base, span) = (i as f32 * 0.4, 0.4f32);
        if tree_done(&tree, k) {
            log(format!("Kernel Debug Kit {} already unpacked", k.build));
            continue;
        }
        let dmg = cache.join(format!("{}.dmg", k.build));
        log(format!("Downloading Kernel Debug Kit {} ({:.2} GB) from {}", k.build, k.size as f64 / 1e9, k.url));
        let stage = format!("Downloading Kernel Debug Kit {}", k.build);
        fetch(k, &dmg, &mut |have, len| progress(base + span * 0.75 * (have as f32 / len.max(1) as f32), &stage))?;
        if tree.exists() {
            std::fs::remove_dir_all(&tree).map_err(|e| format!("cannot clear {}: {e}", tree.display()))?;
        }
        let stage = format!("Unpacking Kernel Debug Kit {}", k.build);
        let r = extract(&dmg, &tree, &mut |done, total| progress(base + span * (0.75 + 0.25 * done as f32 / total.max(1) as f32), &stage))
            .map_err(|e| format!("unpacking {}: {e:#}", k.build))?;
        std::fs::write(tree.join(PIN_FILE), k.sha256).map_err(|e| format!("cannot write {}: {e}", tree.display()))?;
        log(format!("Unpacked {} files ({:.2} GB) from {}", r.files, r.bytes as f64 / 1e9, k.build));
        let _ = std::fs::remove_file(&dmg);
    }

    progress(0.8, "Checking the kernel");
    let (tahoe, k27) = (cache.join(KDK_TAHOE.build), cache.join(KDK_27.build));
    let kernel_path = k27.join(KERNEL_PATH);
    let xkernel = std::fs::read(&kernel_path).map_err(|e| format!("cannot read {}: {e}", kernel_path.display()))?;
    if sha256(&xkernel) != KERNEL_SHA256 {
        return Err(format!("{} is not the kernel anathema-xboot is made for (sha256 differs)", kernel_path.display()));
    }
    let uuid = x86_64_slice(&xkernel).and_then(|s| Macho::parse(s).ok()).and_then(|m| m.uuid());
    if uuid != Some(KERNEL_UUID) {
        return Err(format!("{} has the wrong UUID", kernel_path.display()));
    }
    progress(0.85, "Linking the kext collection");
    let ext = |root: &Path| root.join("System").join("Library").join("Extensions");
    let linked = link(&kc_config(&ext(&tahoe), &ext(&k27), &kernel_path)).map_err(|e| format!("linking the kexts: {e:#}"))?;
    for line in &linked.report.lines {
        log(line.clone());
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    for (name, data) in [("xkernel", &xkernel), ("xkc.bin", &linked.xkc)] {
        std::fs::write(out.join(name), data).map_err(|e| format!("cannot write {name}: {e}"))?;
    }
    write_stamp(&out, &stamp(), &xkernel, &linked.xkc)?;
    for tree in [tahoe, k27] {
        let _ = std::fs::remove_dir_all(tree);
    }
    log(format!("Kernel files ready: xkernel {} bytes, xkc.bin {} bytes", xkernel.len(), linked.xkc.len()));
    progress(1.0, "Kernel files ready");
    Ok(KernelFiles { xkernel, xkc: linked.xkc })
}

/// `prepare_with` downloading from the pinned dortania releases.
pub fn prepare(cache: &Path, free_bytes: u64, progress: &mut dyn FnMut(f32, &str), log: &mut dyn FnMut(String)) -> Result<KernelFiles, String> {
    let mut fetch = |k: &Kdk, path: &Path, p: &mut dyn FnMut(u64, u64)| download_verified(k.url, path, k.size, k.sha256, p).map_err(|e| format!("{e:#}"));
    prepare_with(cache, free_bytes, &mut fetch, progress, log)
}

/// Everything that goes on the EFI partition.
pub fn esp_files<'a>(efi: &'a [u8], k: &'a KernelFiles) -> Vec<FileSpec<'a>> {
    vec![
        FileSpec { path: "EFI/BOOT/BOOTX64.EFI", data: efi },
        FileSpec { path: "anathema/xkernel", data: &k.xkernel },
        FileSpec { path: "anathema/xkc.bin", data: &k.xkc },
        FileSpec { path: "anathema/xcp.bin", data: XCP },
        FileSpec { path: "anathema/xcp2.bin", data: XCP2 },
        FileSpec { path: "anathema/xcp3.bin", data: XCP3 },
    ]
}

/// The order update-boot writes in: the kernel files first and the loader last, so a failure
/// part way leaves the old loader in place.
pub fn update_order<'a>(files: &[FileSpec<'a>]) -> Vec<FileSpec<'a>> {
    let mut v = files.to_vec();
    v.sort_by_key(|f| f.path == "EFI/BOOT/BOOTX64.EFI");
    v
}

/// Checks that updating an existing EFI partition fits, before anything is written. Each file
/// is written next to the old one and then renamed over it, so the old copy is only freed after
/// the new one exists. Sizes are rounded up to 64 KiB for cluster slack.
pub fn update_plan(esp_free: u64, existing: &[(String, u64)], files: &[FileSpec]) -> Result<(), String> {
    const SLACK: u64 = 64 << 10;
    let round = |n: u64| n.div_ceil(SLACK) * SLACK;
    let (mut delta, mut peak) = (0i128, 0i128);
    for f in files {
        let new = round(f.data.len() as u64) as i128;
        let old = existing.iter().find(|(p, _)| p == f.path).map(|(_, n)| round(*n) as i128).unwrap_or(0);
        peak = peak.max(delta + new);
        delta += new - old;
    }
    let need = peak.max(0) as u64 + (1 << 20);
    if need > esp_free {
        return Err(format!("The EFI partition is too small: {} MB needed, {} MB free", need.div_ceil(1 << 20), esp_free >> 20));
    }
    Ok(())
}
