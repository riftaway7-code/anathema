//! Command-line argument parsing for `anathema-test`.

use crate::qemu::BootConfig;
use anyhow::{bail, Context, Result};
use std::path::PathBuf;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cli {
    pub cache_dir: Option<PathBuf>,
    pub out_dir: Option<PathBuf>,
    pub report_path: Option<PathBuf>,
    pub disk_img: Option<PathBuf>,
    pub qemu_path: Option<PathBuf>,
    pub ovmf_code: Option<PathBuf>,
    pub ovmf_vars: Option<PathBuf>,
    pub boot: BootConfig,
    pub skip_tests: bool,
    pub skip_kdk: bool,
    pub skip_boot: bool,
    pub help: bool,
}

pub const USAGE: &str = "\
usage: anathema-test [OPTIONS]

One-command runner that:
  1. builds the anathema-xboot UEFI loader
  2. downloads & unpacks pinned Apple KDKs, links xkc.bin, and builds esp.img
  3. runs all available Rust unit/integration test suites
  4. boots esp.img (or --disk IMAGE) in QEMU (Windows, Linux, macOS) and reads serial + debug buffer
  5. writes a Markdown test report

options:
  --cache DIR       KDK cache directory (default: OS cache dir)
  --out DIR         artifact & log output directory (default: <target>/anathema-test)
  --report FILE     report Markdown output path (default: <out>/report.md)
  --disk FILE       boot a full GPT disk image instead of building/booting esp.img
  --qemu PATH       explicit path to qemu-system-x86_64(.exe)
  --ovmf-code PATH  explicit OVMF/EDK2 code firmware image
  --ovmf-vars PATH  explicit OVMF/EDK2 vars firmware image
  --wait SECS       QEMU boot wait duration in seconds (default: 60)
  --cpu MODEL       QEMU -cpu model (default: Haswell)
  --smp N           QEMU -smp count (default: 1)
  --machine TYPE    QEMU -machine type (default: q35)
  --accel ACCEL     QEMU -accel backend (default: tcg)
  --skip-tests      skip cargo test suites
  --skip-kdk        skip KDK download/link and ESP image build (implies --skip-boot unless --disk)
  --skip-boot       skip QEMU kernel boot attempt
  -h, --help        show this help";

pub fn parse_args(args: &[String]) -> Result<Cli> {
    let mut cli = Cli::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut next = || -> Result<String> {
            i += 1;
            args.get(i).cloned().with_context(|| format!("{a} needs a value"))
        };
        match a {
            "-h" | "--help" => cli.help = true,
            "--cache" => cli.cache_dir = Some(PathBuf::from(next()?)),
            "--out" => cli.out_dir = Some(PathBuf::from(next()?)),
            "--report" => cli.report_path = Some(PathBuf::from(next()?)),
            "--disk" => cli.disk_img = Some(PathBuf::from(next()?)),
            "--qemu" => cli.qemu_path = Some(PathBuf::from(next()?)),
            "--ovmf-code" => cli.ovmf_code = Some(PathBuf::from(next()?)),
            "--ovmf-vars" => cli.ovmf_vars = Some(PathBuf::from(next()?)),
            "--wait" => cli.boot.wait_secs = next()?.parse().context("--wait must be seconds")?,
            "--cpu" => cli.boot.cpu = next()?,
            "--smp" => cli.boot.smp = next()?.parse().context("--smp must be an integer")?,
            "--machine" => cli.boot.machine = next()?,
            "--accel" => cli.boot.accel = next()?,
            "--skip-tests" => cli.skip_tests = true,
            "--skip-kdk" => cli.skip_kdk = true,
            "--skip-boot" => cli.skip_boot = true,
            other => bail!("unknown argument `{other}` (pass --help for usage)"),
        }
        i += 1;
    }
    Ok(cli)
}
