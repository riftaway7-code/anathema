use anathema_test::cli::parse_args;
use anathema_test::env::pick_first_existing_pair;
use anathema_test::qemu::{analyze_boot_output, clean_monitor_output, parse_monitor_chars, parse_monitor_hex64};
use anathema_test::report::{render_markdown, ReportInput};
use anathema_test::runner::{summarize_test_output, StepResult, StepStatus};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn parses_cli_flags() {
    let args: Vec<String> = [
        "--disk", "full.img", "--wait", "90", "--cpu", "Skylake-Client", "--smp", "2", "--accel", "whpx", "--skip-boot",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let cli = parse_args(&args).unwrap();
    assert_eq!(cli.disk_img, Some(PathBuf::from("full.img")));
    assert_eq!(cli.boot.wait_secs, 90);
    assert_eq!(cli.boot.cpu, "Skylake-Client");
    assert_eq!(cli.boot.smp, 2);
    assert_eq!(cli.boot.accel, "whpx");
    assert!(cli.skip_boot);
    assert!(!cli.skip_tests);
    assert!(parse_args(&["--unknown".to_string()]).is_err());
}

#[test]
fn parses_qemu_monitor_hex64() {
    let raw = "(qemu) x/gx 0xffffff8000e99b08\r\nffffff8000e99b08: 0xffffff8000f12340\r\n(qemu) ";
    assert_eq!(parse_monitor_hex64(raw), Some(0xffffff8000f12340));
    assert_eq!(parse_monitor_hex64("no address here"), None);
}

#[test]
fn parses_qemu_monitor_char_dump() {
    let raw = "(qemu) x/8bc 0xffffff8000f12340\r\n\
               ffffff8000f12340: 'h' 'i' '\\n' '\\'' '\\\\' '\\x41' '\\x00' '!'";
    assert_eq!(parse_monitor_chars(raw), "hi\n'\\A!");
}

#[test]
fn detects_boot_milestones_and_panic() {
    let serial = "\
anathema-xboot: start
anathema-xboot: kext block installed
anathema-xboot: exiting boot services
anathema-xboot: kernel copied, jumping
Waiting on <dict ID=\"0\"><key>IOProviderClass</key></dict>
";
    let dbg = "Darwin Kernel Version 27.2.0\npanic(cpu 0 caller 0xffffff8000123456): test panic\n";
    let m = analyze_boot_output(serial, dbg);
    assert!(m.xboot_started);
    assert!(m.kext_block_installed);
    assert!(m.exiting_boot_services);
    assert!(m.kernel_jumped);
    assert!(m.waiting_for_root);
    assert!(!m.userspace_reached);
    assert!(m.kernel_panic);
    assert_eq!(
        m.panic_line.as_deref(),
        Some("panic(cpu 0 caller 0xffffff8000123456): test panic")
    );

    let full = analyze_boot_output("anathema-xboot: start\nanx loadkext tmpfs: 0\nhello, launchd", "");
    assert!(full.userspace_reached);
}

#[test]
fn strips_ansi_and_prompt_from_monitor_output() {
    let raw = "(qemu) \x1b[Ki\x1b[Dinfo registers\r\nRAX=0000000000000001 RBX=0000000000000002\r\n(qemu) ";
    assert_eq!(
        clean_monitor_output(raw, "info registers"),
        "RAX=0000000000000001 RBX=0000000000000002"
    );
}

#[test]
fn summarizes_cargo_test_output() {
    let out = "running 3 tests\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";
    assert_eq!(
        summarize_test_output(out, Some(0)),
        "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    );
    assert_eq!(summarize_test_output("", Some(101)), "exit code 101");
}

#[test]
fn picks_first_existing_firmware_pair() {
    let base = std::env::temp_dir().join(format!("anathema-test-fw-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let c = base.join("code.fd");
    let v = base.join("vars.fd");
    std::fs::write(&c, b"code").unwrap();
    std::fs::write(&v, b"vars").unwrap();

    let picked = pick_first_existing_pair(&[
        (base.join("missing.fd"), base.join("missing_v.fd")),
        (c.clone(), v.clone()),
    ])
    .unwrap();
    assert_eq!(picked.code, c);
    assert_eq!(picked.vars, v);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn renders_markdown_report() {
    let host = anathema_test::env::HostInfo {
        os: "windows".into(),
        arch: "x86_64".into(),
        repo_root: PathBuf::from(r"C:\repo\anathema"),
        cache_dir: PathBuf::from(r"C:\cache\kdk"),
        cargo_version: Some("cargo 1.99.1".into()),
        rustc_version: Some("rustc 1.99.1".into()),
        uefi_target_installed: true,
        qemu_path: None,
        qemu_version: None,
        firmware: None,
    };
    let steps = vec![StepResult {
        name: "anathema-media tests".into(),
        command: "cargo test -p anathema-media".into(),
        status: StepStatus::Passed,
        duration: Duration::from_secs(2),
        summary: "test result: ok. 12 passed; 0 failed".into(),
        output: String::new(),
    }];
    let md = render_markdown(&ReportInput {
        host: &host,
        steps: &steps,
        boot: None,
        boot_skip_reason: Some("qemu-system-x86_64 not found"),
        boot_image: None,
    });
    assert!(md.contains("# Anathema Automated Test Report"));
    assert!(md.contains("| anathema-media tests | PASSED | 2.0s |"));
    assert!(md.contains("SKIPPED (qemu-system-x86_64 not found)"));
}
