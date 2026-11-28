//! Subprocess execution for building components, preparing KDK/ESP, and running cargo tests.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Passed,
    Failed,
    Skipped,
}

impl StepStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            StepStatus::Passed => "PASSED",
            StepStatus::Failed => "FAILED",
            StepStatus::Skipped => "SKIPPED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub command: String,
    pub status: StepStatus,
    pub duration: Duration,
    pub summary: String,
    pub output: String,
}

pub fn run_command_step(
    name: &str,
    program: &str,
    args: &[&str],
    cwd: &Path,
    envs: &[(&str, &Path)],
) -> StepResult {
    let display = format!("{program} {}", args.join(" "));
    let start = Instant::now();
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output();
    let duration = start.elapsed();
    match out {
        Ok(o) => {
            let mut combined = String::from_utf8_lossy(&o.stdout).into_owned();
            if !o.stderr.is_empty() {
                if !combined.is_empty() && !combined.ends_with('\n') {
                    combined.push('\n');
                }
                combined.push_str(&String::from_utf8_lossy(&o.stderr));
            }
            let status = if o.status.success() { StepStatus::Passed } else { StepStatus::Failed };
            let summary = summarize_test_output(&combined, o.status.code());
            StepResult {
                name: name.to_string(),
                command: display,
                status,
                duration,
                summary,
                output: combined,
            }
        }
        Err(e) => StepResult {
            name: name.to_string(),
            command: display,
            status: StepStatus::Failed,
            duration,
            summary: format!("failed to spawn: {e}"),
            output: format!("failed to spawn `{program}`: {e}\n"),
        },
    }
}

/// Summarize `cargo test` output into a single concise line.
pub fn summarize_test_output(output: &str, exit_code: Option<i32>) -> String {
    let result_lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("test result:"))
        .collect();
    if !result_lines.is_empty() {
        return result_lines.join(" | ");
    }
    match exit_code {
        Some(0) => "ok".to_string(),
        Some(c) => format!("exit code {c}"),
        None => "terminated by signal".to_string(),
    }
}

/// Resolve the Cargo target directory (`CARGO_TARGET_DIR` or `<repo_root>/target`).
pub fn target_dir(repo_root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("target"))
}

/// Build `anathema-xboot` for `x86_64-unknown-uefi` and return the path to the `.efi`.
pub fn build_xboot(repo_root: &Path) -> Result<(StepResult, PathBuf)> {
    let step = run_command_step(
        "build anathema-xboot",
        "cargo",
        &["build", "--release", "-p", "anathema-xboot", "--target", "x86_64-unknown-uefi"],
        repo_root,
        &[],
    );
    let efi = target_dir(repo_root)
        .join("x86_64-unknown-uefi")
        .join("release")
        .join("anathema-xboot.efi");
    if step.status != StepStatus::Passed {
        return Ok((step, efi));
    }
    if !efi.is_file() {
        bail!("anathema-xboot built, but {} was not found", efi.display());
    }
    Ok((step, efi))
}

/// Download/prepare KDK files, link `xkc.bin`, and build `esp.img` via `anathema-esp`.
pub fn prepare_esp_image(
    repo_root: &Path,
    efi_path: &Path,
    cache_dir: &Path,
    esp_out: &Path,
) -> Result<StepResult> {
    if let Some(parent) = esp_out.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let efi_str = efi_path.to_string_lossy().into_owned();
    let cache_str = cache_dir.to_string_lossy().into_owned();
    let out_str = esp_out.to_string_lossy().into_owned();
    let args = [
        "run",
        "--release",
        "-p",
        "anathema-setup",
        "--bin",
        "anathema-esp",
        "--",
        "--efi",
        &efi_str,
        "--cache",
        &cache_str,
        "--out",
        &out_str,
    ];
    // `anathema-setup/build.rs` requires `ANATHEMA_EFI` on Windows builds.
    let step = run_command_step(
        "prepare KDKs & build ESP image",
        "cargo",
        &args,
        repo_root,
        &[("ANATHEMA_EFI", efi_path)],
    );
    Ok(step)
}

/// Run all available workspace test suites.
pub fn run_test_suites(repo_root: &Path, efi_path: Option<&Path>) -> Vec<StepResult> {
    let mut results = Vec::new();
    let cross_suites: [(&str, &[&str]); 3] = [
        ("anathema-media tests", &["test", "-p", "anathema-media"]),
        ("anathema-kc tests", &["test", "-p", "anathema-kc"]),
        ("anathema-test self tests", &["test", "-p", "anathema-test", "--lib", "--test", "report_and_qemu"]),
    ];
    for (name, args) in cross_suites {
        results.push(run_command_step(name, "cargo", args, repo_root, &[]));
    }

    let unix_suites: [(&str, &[&str]); 2] = [
        ("anathema-xlat tests", &["test", "-p", "anathema-xlat"]),
        ("anathema-rt MMU/memory tests", &["test", "-p", "anathema-rt", "--test", "mmu_and_memory"]),
    ];
    for (name, args) in unix_suites {
        if cfg!(windows) {
            results.push(StepResult {
                name: name.to_string(),
                command: format!("cargo {}", args.join(" ")),
                status: StepStatus::Skipped,
                duration: Duration::ZERO,
                summary: "skipped on Windows: Design A runtime requires Unix mmap/sigaction".to_string(),
                output: String::new(),
            });
        } else {
            results.push(run_command_step(name, "cargo", args, repo_root, &[]));
        }
    }

    match efi_path {
        Some(efi) if efi.is_file() => {
            results.push(run_command_step(
                "anathema-setup tests",
                "cargo",
                &["test", "-p", "anathema-setup"],
                repo_root,
                &[("ANATHEMA_EFI", efi)],
            ));
        }
        _ => {
            results.push(StepResult {
                name: "anathema-setup tests".to_string(),
                command: "cargo test -p anathema-setup".to_string(),
                status: StepStatus::Skipped,
                duration: Duration::ZERO,
                summary: "skipped: anathema-xboot.efi not built (required for ANATHEMA_EFI on Windows)".to_string(),
                output: String::new(),
            });
        }
    }
    results
}
