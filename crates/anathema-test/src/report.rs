//! Markdown report generation for automated test and kernel-boot runs.

use crate::env::HostInfo;
use crate::qemu::BootResult;
use crate::runner::{StepResult, StepStatus};
use std::path::Path;

pub struct ReportInput<'a> {
    pub host: &'a HostInfo,
    pub steps: &'a [StepResult],
    pub boot: Option<&'a BootResult>,
    pub boot_skip_reason: Option<&'a str>,
    pub boot_image: Option<&'a Path>,
}

/// Render the full Markdown report.
pub fn render_markdown(input: &ReportInput<'_>) -> String {
    let mut md = String::new();
    md.push_str("# Anathema Automated Test Report\n\n");

    md.push_str("## Environment\n\n");
    md.push_str(&format!("- **Host**: `{}` (`{}`)\n", input.host.os, input.host.arch));
    md.push_str(&format!("- **Repo**: `{}`\n", input.host.repo_root.display()));
    md.push_str(&format!("- **KDK Cache**: `{}`\n", input.host.cache_dir.display()));
    md.push_str(&format!(
        "- **Cargo / Rustc**: `{}` / `{}`\n",
        input.host.cargo_version.as_deref().unwrap_or("not found"),
        input.host.rustc_version.as_deref().unwrap_or("not found")
    ));
    md.push_str(&format!(
        "- **UEFI target (`x86_64-unknown-uefi`)**: {}\n",
        if input.host.uefi_target_installed { "installed" } else { "missing" }
    ));
    md.push_str(&format!(
        "- **QEMU**: {}\n",
        input.host.qemu_version.as_deref().unwrap_or("not found")
    ));
    if let Some(fw) = &input.host.firmware {
        md.push_str(&format!(
            "- **UEFI Firmware**: `{}` + `{}`\n",
            fw.code.display(),
            fw.vars.display()
        ));
    } else {
        md.push_str("- **UEFI Firmware**: not found\n");
    }
    if let Some(img) = input.boot_image {
        md.push_str(&format!("- **Boot Image**: `{}`\n", img.display()));
    }

    md.push_str("\n## Build & Test Steps\n\n");
    md.push_str("| Step | Status | Duration | Summary |\n");
    md.push_str("| --- | --- | --- | --- |\n");
    for s in input.steps {
        md.push_str(&format!(
            "| {} | {} | {:.1}s | {} |\n",
            s.name,
            s.status.as_str(),
            s.duration.as_secs_f64(),
            s.summary.replace('|', "\\|")
        ));
    }

    let failed: Vec<&StepResult> = input
        .steps
        .iter()
        .filter(|s| s.status == StepStatus::Failed)
        .collect();
    if !failed.is_empty() {
        md.push_str("\n### Failed Step Logs\n\n");
        for s in failed {
            md.push_str(&format!("#### {}\n\n```text\n{}\n```\n\n", s.name, tail_lines(&s.output, 80)));
        }
    }

    md.push_str("\n## QEMU Kernel Boot Attempt\n\n");
    if let Some(b) = input.boot {
        md.push_str(&format!("- **Duration**: {:.1}s\n", b.duration.as_secs_f64()));
        md.push_str(&format!("- **xboot started**: {}\n", b.milestones.xboot_started));
        md.push_str(&format!("- **kext block installed**: {}\n", b.milestones.kext_block_installed));
        md.push_str(&format!("- **exiting boot services**: {}\n", b.milestones.exiting_boot_services));
        md.push_str(&format!("- **kernel jumped**: {}\n", b.milestones.kernel_jumped));
        md.push_str(&format!("- **waiting for root device**: {}\n", b.milestones.waiting_for_root));
        md.push_str(&format!("- **userspace reached**: {}\n", b.milestones.userspace_reached));
        md.push_str(&format!("- **kernel panic detected**: {}\n", b.milestones.kernel_panic));
        if let Some(p) = &b.milestones.panic_line {
            md.push_str(&format!("- **Panic line**: `{p}`\n"));
        }
        if !b.registers.is_empty() {
            let preview: String = b.registers.chars().take(400).collect();
            md.push_str(&format!("\n### Registers (preview)\n\n```text\n{preview}\n```\n"));
        }
        if !b.debug_buf.is_empty() {
            md.push_str(&format!("\n### Kernel Debug Buffer\n\n```text\n{}\n```\n", b.debug_buf.trim_end()));
        }
        if !b.serial_log.is_empty() {
            md.push_str(&format!("\n### Serial Log (tail)\n\n```text\n{}\n```\n", tail_lines(&b.serial_log, 80)));
        }
    } else {
        let reason = input.boot_skip_reason.unwrap_or("skipped");
        md.push_str(&format!("- **Status**: SKIPPED ({reason})\n"));
    }

    md
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}
