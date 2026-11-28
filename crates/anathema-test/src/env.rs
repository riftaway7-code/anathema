//! Host environment detection and tool/firmware discovery (Windows, Linux, macOS).

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwarePair {
    pub code: PathBuf,
    pub vars: PathBuf,
}

#[derive(Debug, Clone)]
pub struct HostInfo {
    pub os: String,
    pub arch: String,
    pub repo_root: PathBuf,
    pub cache_dir: PathBuf,
    pub cargo_version: Option<String>,
    pub rustc_version: Option<String>,
    pub uefi_target_installed: bool,
    pub qemu_path: Option<PathBuf>,
    pub qemu_version: Option<String>,
    pub firmware: Option<FirmwarePair>,
}

/// Find the workspace root by walking upward from `CARGO_MANIFEST_DIR` or current working directory.
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        let cargo = dir.join("Cargo.toml");
        if cargo.is_file() && dir.join("crates").join("anathema-xboot").is_dir() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

/// Cross-platform default KDK cache directory.
pub fn default_cache_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("Anathema").join("kdk");
        }
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Library").join("Caches").join("anathema").join("kdk");
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(xdg).join("anathema").join("kdk");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("anathema").join("kdk");
    }
    std::env::temp_dir().join("anathema-kdk")
}

/// Locate `qemu-system-x86_64` from an explicit path, `PATH`, or common platform install locations.
pub fn find_qemu(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return p.is_file().then(|| p.to_path_buf());
    }
    let exe = if cfg!(windows) { "qemu-system-x86_64.exe" } else { "qemu-system-x86_64" };
    if let Some(found) = find_on_path(exe) {
        return Some(found);
    }
    let candidates = qemu_candidates();
    candidates.into_iter().find(|p| p.is_file())
}

fn qemu_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if cfg!(windows) {
        for key in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(key) {
                let b = PathBuf::from(base);
                out.push(b.join("qemu").join("qemu-system-x86_64.exe"));
                out.push(b.join("QEMU").join("qemu-system-x86_64.exe"));
            }
        }
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let h = PathBuf::from(home);
            out.push(h.join("scoop").join("shims").join("qemu-system-x86_64.exe"));
            out.push(h.join("scoop").join("apps").join("qemu").join("current").join("qemu-system-x86_64.exe"));
        }
        out.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin\qemu-system-x86_64.exe"));
    } else {
        out.push(PathBuf::from("/usr/bin/qemu-system-x86_64"));
        out.push(PathBuf::from("/usr/local/bin/qemu-system-x86_64"));
        out.push(PathBuf::from("/opt/homebrew/bin/qemu-system-x86_64"));
    }
    out
}

/// Locate an executable on `PATH`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let full = dir.join(name);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

/// Discover OVMF / EDK2 x86_64 code and vars firmware files.
pub fn find_firmware(
    explicit_code: Option<&Path>,
    explicit_vars: Option<&Path>,
    qemu_path: Option<&Path>,
) -> Option<FirmwarePair> {
    if let (Some(c), Some(v)) = (explicit_code, explicit_vars) {
        if c.is_file() && v.is_file() {
            return Some(FirmwarePair { code: c.to_path_buf(), vars: v.to_path_buf() });
        }
        return None;
    }
    if let (Ok(c), Ok(v)) = (std::env::var("OVMF_CODE"), std::env::var("OVMF_VARS")) {
        let cp = PathBuf::from(c);
        let vp = PathBuf::from(v);
        if cp.is_file() && vp.is_file() {
            return Some(FirmwarePair { code: cp, vars: vp });
        }
    }
    let pairs = candidate_firmware_pairs(qemu_path);
    pick_first_existing_pair(&pairs)
}

/// Pick the first firmware pair whose files both exist on disk.
pub fn pick_first_existing_pair(pairs: &[(PathBuf, PathBuf)]) -> Option<FirmwarePair> {
    for (c, v) in pairs {
        if c.is_file() && v.is_file() {
            return Some(FirmwarePair { code: c.clone(), vars: v.clone() });
        }
    }
    None
}

fn candidate_firmware_pairs(qemu_path: Option<&Path>) -> Vec<(PathBuf, PathBuf)> {
    let mut pairs = Vec::new();
    if let Some(q) = qemu_path {
        if let Some(qdir) = q.parent() {
            let mut bases = vec![qdir.to_path_buf(), qdir.join("share")];
            if let Some(parent) = qdir.parent() {
                bases.push(parent.join("share"));
                bases.push(parent.join("share").join("qemu"));
            }
            for b in bases {
                pairs.push((b.join("edk2-x86_64-code.fd"), b.join("edk2-i386-vars.fd")));
                pairs.push((b.join("edk2-x86_64-code.fd"), b.join("edk2-x86_64-vars.fd")));
                pairs.push((b.join("OVMF_CODE_4M.fd"), b.join("OVMF_VARS_4M.fd")));
                pairs.push((b.join("OVMF_CODE.fd"), b.join("OVMF_VARS.fd")));
            }
        }
    }
    pairs.extend([
        (PathBuf::from("/usr/share/OVMF/OVMF_CODE_4M.fd"), PathBuf::from("/usr/share/OVMF/OVMF_VARS_4M.fd")),
        (PathBuf::from("/usr/share/OVMF/OVMF_CODE.fd"), PathBuf::from("/usr/share/OVMF/OVMF_VARS.fd")),
        (PathBuf::from("/usr/share/edk2/x64/OVMF_CODE.4m.fd"), PathBuf::from("/usr/share/edk2/x64/OVMF_VARS.4m.fd")),
        (PathBuf::from("/opt/homebrew/share/qemu/edk2-x86_64-code.fd"), PathBuf::from("/opt/homebrew/share/qemu/edk2-i386-vars.fd")),
        (PathBuf::from("/usr/local/share/qemu/edk2-x86_64-code.fd"), PathBuf::from("/usr/local/share/qemu/edk2-i386-vars.fd")),
    ]);
    pairs
}

fn command_first_line(bin: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).lines().next().map(|s| s.trim().to_string())
}

fn check_uefi_target() -> bool {
    let Ok(out) = Command::new("rustup").args(["target", "list", "--installed"]).output() else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    String::from_utf8_lossy(&out.stdout).lines().any(|l| l.trim() == "x86_64-unknown-uefi")
}

/// Probe host environment, toolchain, QEMU, and firmware availability.
pub fn probe_host(
    repo_root: &Path,
    cache_dir: &Path,
    explicit_qemu: Option<&Path>,
    explicit_code: Option<&Path>,
    explicit_vars: Option<&Path>,
) -> HostInfo {
    let qemu_path = find_qemu(explicit_qemu);
    let qemu_version = qemu_path.as_deref().and_then(|q| command_first_line(q, &["--version"]));
    let firmware = find_firmware(explicit_code, explicit_vars, qemu_path.as_deref());
    HostInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        repo_root: repo_root.to_path_buf(),
        cache_dir: cache_dir.to_path_buf(),
        cargo_version: command_first_line(Path::new("cargo"), &["--version"]),
        rustc_version: command_first_line(Path::new("rustc"), &["--version"]),
        uefi_target_installed: check_uefi_target(),
        qemu_path,
        qemu_version,
        firmware,
    }
}
