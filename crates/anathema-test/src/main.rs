//! `anathema-test`: one-command cross-platform test, KDK/ESP setup, QEMU boot, and report tool.

use anathema_test::cli::{parse_args, USAGE};
use anathema_test::env::{default_cache_dir, find_repo_root, probe_host};
use anathema_test::qemu::run_qemu_boot;
use anathema_test::report::{render_markdown, ReportInput};
use anathema_test::runner::{build_xboot, prepare_esp_image, run_test_suites, target_dir, StepResult, StepStatus};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(ok) => {
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("anathema-test: error: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<bool> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = parse_args(&args)?;
    if cli.help {
        println!("{USAGE}");
        return Ok(true);
    }

    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)
        .or_else(|| find_repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))))
        .context("could not locate the anathema workspace root")?;
    let cache_dir = cli.cache_dir.unwrap_or_else(default_cache_dir);
    let out_dir = cli
        .out_dir
        .unwrap_or_else(|| target_dir(&repo_root).join("anathema-test"));
    std::fs::create_dir_all(&out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let report_path = cli.report_path.unwrap_or_else(|| out_dir.join("report.md"));
    let esp_img = out_dir.join("esp.img");

    let host = probe_host(
        &repo_root,
        &cache_dir,
        cli.qemu_path.as_deref(),
        cli.ovmf_code.as_deref(),
        cli.ovmf_vars.as_deref(),
    );
    if host.cargo_version.is_none() {
        bail!("`cargo` was not found on PATH; install Rust (https://rustup.rs) first");
    }

    let mut steps: Vec<StepResult> = Vec::new();
    let mut efi_path: Option<PathBuf> = None;

    if host.uefi_target_installed {
        println!("[1/4] Building anathema-xboot (x86_64-unknown-uefi)...");
        let (step, efi) = build_xboot(&repo_root)?;
        println!("      {} ({:.1}s)", step.status.as_str(), step.duration.as_secs_f64());
        if step.status == StepStatus::Passed {
            efi_path = Some(efi);
        }
        steps.push(step);
    } else {
        println!("[1/4] Skipping anathema-xboot (run `rustup target add x86_64-unknown-uefi` to enable)");
        steps.push(StepResult {
            name: "build anathema-xboot".to_string(),
            command: "cargo build --release -p anathema-xboot --target x86_64-unknown-uefi".to_string(),
            status: StepStatus::Skipped,
            duration: Default::default(),
            summary: "skipped: rustup target x86_64-unknown-uefi is not installed".to_string(),
            output: String::new(),
        });
    }

    let mut boot_image: Option<PathBuf> = None;
    if let Some(disk) = cli.disk_img {
        if !disk.is_file() {
            bail!("--disk image does not exist: {}", disk.display());
        }
        println!("[2/4] Using provided disk image {} (skipping KDK/ESP build)", disk.display());
        boot_image = Some(disk);
    } else if cli.skip_kdk {
        println!("[2/4] Skipping KDK download & ESP image (--skip-kdk)");
    } else if let Some(efi) = efi_path.as_deref() {
        println!("[2/4] Preparing KDKs, linking xkc.bin, and building {}...", esp_img.display());
        let step = prepare_esp_image(&repo_root, efi, &cache_dir, &esp_img)?;
        println!("      {} ({:.1}s)", step.status.as_str(), step.duration.as_secs_f64());
        if step.status == StepStatus::Passed && esp_img.is_file() {
            boot_image = Some(esp_img.clone());
        }
        steps.push(step);
    } else {
        println!("[2/4] Skipping KDK & ESP image (anathema-xboot.efi unavailable)");
        steps.push(StepResult {
            name: "prepare KDKs & build ESP image".to_string(),
            command: "cargo run --release -p anathema-setup --bin anathema-esp".to_string(),
            status: StepStatus::Skipped,
            duration: Default::default(),
            summary: "skipped: anathema-xboot.efi not built".to_string(),
            output: String::new(),
        });
    }

    if cli.skip_tests {
        println!("[3/4] Skipping workspace unit tests (--skip-tests)");
    } else {
        println!("[3/4] Running workspace test suites...");
        for s in run_test_suites(&repo_root, efi_path.as_deref()) {
            println!("      {:<30} {} ({})", s.name, s.status.as_str(), s.summary);
            steps.push(s);
        }
    }

    let mut boot_res = None;
    let mut boot_skip: Option<String> = None;
    if cli.skip_boot || (cli.skip_kdk && boot_image.is_none()) {
        boot_skip = Some("disabled by CLI flag".to_string());
        println!("[4/4] Skipping QEMU kernel boot");
    } else if let Some(img) = boot_image.as_deref() {
        if let (Some(q), Some(fw)) = (host.qemu_path.as_deref(), host.firmware.as_ref()) {
            println!("[4/4] Booting {} in QEMU ({}s wait)...", img.display(), cli.boot.wait_secs);
            match run_qemu_boot(q, fw, img, &out_dir.join("qemu"), &cli.boot) {
                Ok(b) => {
                    println!(
                        "      booted: xboot={}, kext_block={}, jumped={}, waiting_root={}, userspace={}, panic={}",
                        b.milestones.xboot_started,
                        b.milestones.kext_block_installed,
                        b.milestones.kernel_jumped,
                        b.milestones.waiting_for_root,
                        b.milestones.userspace_reached,
                        b.milestones.kernel_panic
                    );
                    boot_res = Some(b);
                }
                Err(e) => {
                    boot_skip = Some(format!("QEMU launch failed: {e:#}"));
                    eprintln!("      QEMU error: {e:#}");
                }
            }
        } else {
            let missing = if host.qemu_path.is_none() {
                "qemu-system-x86_64 not found (install QEMU or pass --qemu)"
            } else {
                "OVMF/EDK2 firmware not found (pass --ovmf-code/--ovmf-vars or set OVMF_CODE/OVMF_VARS)"
            };
            boot_skip = Some(missing.to_string());
            println!("[4/4] Skipping QEMU kernel boot ({missing})");
        }
    } else {
        boot_skip = Some("ESP image was not built".to_string());
        println!("[4/4] Skipping QEMU kernel boot (ESP image not built)");
    }

    let md = render_markdown(&ReportInput {
        host: &host,
        steps: &steps,
        boot: boot_res.as_ref(),
        boot_skip_reason: boot_skip.as_deref(),
        boot_image: boot_image.as_deref(),
    });
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, &md).with_context(|| format!("writing {}", report_path.display()))?;
    println!("Report written to {}", report_path.display());

    let any_failed = steps.iter().any(|s| s.status == StepStatus::Failed);
    Ok(!any_failed)
}
