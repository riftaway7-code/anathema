//! Cross-platform QEMU boot runner using a TCP QEMU monitor (works on Windows, Linux, and macOS).

use crate::env::FirmwarePair;
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootConfig {
    pub machine: String,
    pub cpu: String,
    pub smp: u32,
    pub accel: String,
    pub wait_secs: u64,
}

impl Default for BootConfig {
    fn default() -> Self {
        Self {
            machine: "q35".to_string(),
            cpu: "Haswell".to_string(),
            smp: 1,
            accel: "tcg".to_string(),
            wait_secs: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootMilestones {
    pub xboot_started: bool,
    pub kext_block_installed: bool,
    pub exiting_boot_services: bool,
    pub kernel_jumped: bool,
    pub waiting_for_root: bool,
    pub userspace_reached: bool,
    pub kernel_panic: bool,
    pub panic_line: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BootResult {
    pub duration: Duration,
    pub registers: String,
    pub debug_buf: String,
    pub serial_log: String,
    pub milestones: BootMilestones,
}

fn free_tcp_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("binding ephemeral port for QEMU monitor")?;
    Ok(listener.local_addr()?.port())
}

struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Strip ANSI CSI sequences and carriage returns from QEMU monitor text, and drop any echoed prompt/command prefix.
pub fn clean_monitor_output(raw: &str, cmd: &str) -> String {
    let mut cleaned = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else if c != '\r' {
            cleaned.push(c);
        }
    }
    let mut lines: Vec<&str> = cleaned.lines().collect();
    if let Some(first) = lines.first() {
        if first.contains("(qemu)") || (!cmd.is_empty() && first.contains(cmd)) {
            lines.remove(0);
        }
    }
    if let Some(last) = lines.last() {
        if last.trim() == "(qemu)" {
            lines.pop();
        }
    }
    lines.join("\n")
}

fn read_until_prompt(stream: &mut TcpStream, timeout: Duration) -> String {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    let deadline = Instant::now() + timeout;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    while Instant::now() < deadline {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if out.ends_with(b"(qemu) ") || out.len() >= 1 << 20 {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                if out.ends_with(b"(qemu) ") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn monitor_cmd(stream: &mut TcpStream, cmd: &str) -> String {
    let _ = stream.write_all(format!("{cmd}\n").as_bytes());
    let _ = stream.flush();
    let raw = read_until_prompt(stream, Duration::from_secs(3));
    clean_monitor_output(&raw, cmd)
}

/// Parse the last `0x...` pointer printed by QEMU monitor `x/gx 0xffffff...`.
pub fn parse_monitor_hex64(output: &str) -> Option<u64> {
    let mut last = None;
    for line in output.lines() {
        if let Some((_, rhs)) = line.split_once(':') {
            for tok in rhs.split_whitespace() {
                if let Some(hex) = tok.strip_prefix("0x") {
                    if let Ok(v) = u64::from_str_radix(hex.trim_end_matches(','), 16) {
                        last = Some(v);
                    }
                }
            }
        }
    }
    last
}

/// Parse character literals emitted by QEMU monitor `x/<N>bc <addr>`.
pub fn parse_monitor_chars(output: &str) -> String {
    let mut out = String::new();
    let bytes = output.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if let Some(end_rel) = find_closing_quote(&bytes[i + 1..]) {
                let inner = &output[i + 1..i + 1 + end_rel];
                decode_char_token(inner, &mut out);
                i += 1 + end_rel + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn find_closing_quote(slice: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < slice.len() {
        if slice[i] == b'\\' && i + 1 < slice.len() {
            i += 2;
            continue;
        }
        if slice[i] == b'\'' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn decode_char_token(tok: &str, out: &mut String) {
    match tok {
        "\\n" => out.push('\n'),
        "\\r" => {}
        "\\t" => out.push('\t'),
        "\\'" => out.push('\''),
        "\\\\" => out.push('\\'),
        s if s.starts_with("\\x") && s.len() == 4 => {
            if let Ok(b) = u8::from_str_radix(&s[2..], 16) {
                if b.is_ascii_graphic() || b == b' ' {
                    out.push(b as char);
                }
            }
        }
        s => out.push_str(s),
    }
}

/// Analyze serial output and kernel debug buffer for Anathema boot milestones.
pub fn analyze_boot_output(serial: &str, debug_buf: &str) -> BootMilestones {
    let combined = format!("{serial}\n{debug_buf}");
    let panic_line = combined
        .lines()
        .find(|l| l.contains("panic(cpu") || l.contains("Debugger called:"))
        .map(|l| l.trim().to_string());
    BootMilestones {
        xboot_started: serial.contains("anathema-xboot: start") || serial.contains("anathema-xboot: native"),
        kext_block_installed: serial.contains("anathema-xboot: kext block installed"),
        exiting_boot_services: serial.contains("anathema-xboot: exiting boot services"),
        kernel_jumped: serial.contains("anathema-xboot: kernel copied, jumping"),
        waiting_for_root: serial.contains("Waiting on <dict") || serial.contains("Still waiting for root device"),
        userspace_reached: serial.contains("anx loadkext")
            || serial.contains("anx sandbox")
            || serial.contains("hello, launchd")
            || serial.contains("BaseSystem environment"),
        kernel_panic: panic_line.is_some(),
        panic_line,
    }
}

/// Launch QEMU with the ESP image as an emulated USB stick, capture serial + debug buffer.
pub fn run_qemu_boot(
    qemu_bin: &Path,
    fw: &FirmwarePair,
    esp_img: &Path,
    run_dir: &Path,
    cfg: &BootConfig,
) -> Result<BootResult> {
    std::fs::create_dir_all(run_dir).with_context(|| format!("creating {}", run_dir.display()))?;
    let vars_copy = run_dir.join("ovmf_vars.fd");
    std::fs::copy(&fw.vars, &vars_copy)
        .with_context(|| format!("copying {} to {}", fw.vars.display(), vars_copy.display()))?;
    let serial_path = run_dir.join("serial.log");
    let _ = std::fs::remove_file(&serial_path);
    let port = free_tcp_port()?;

    let pflash_code = format!("if=pflash,format=raw,readonly=on,file={}", fw.code.display());
    let pflash_vars = format!("if=pflash,format=raw,file={}", vars_copy.display());
    let usb_drive = format!("file={},format=raw,if=none,id=usbstick,snapshot=on", esp_img.display());
    let serial_arg = format!("file:{}", serial_path.display());
    let monitor_arg = format!("tcp:127.0.0.1:{port},server,nowait");

    let start = Instant::now();
    let child = Command::new(qemu_bin)
        .args([
            "-machine",
            &cfg.machine,
            "-cpu",
            &cfg.cpu,
            "-m",
            "2G",
            "-smp",
            &cfg.smp.to_string(),
            "-accel",
            &cfg.accel,
            "-display",
            "none",
            "-no-reboot",
            "-drive",
            &pflash_code,
            "-drive",
            &pflash_vars,
            "-device",
            "qemu-xhci,id=xhci",
            "-drive",
            &usb_drive,
            "-device",
            "usb-storage,bus=xhci.0,drive=usbstick,bootindex=0",
            "-serial",
            &serial_arg,
            "-monitor",
            &monitor_arg,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawning {}", qemu_bin.display()))?;
    let mut guard = ChildGuard(Some(child));

    let deadline = Instant::now() + Duration::from_secs(cfg.wait_secs);
    while Instant::now() < deadline {
        if let Some(c) = guard.0.as_mut() {
            if c.try_wait()?.is_some() {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let mut registers = String::new();
    let mut debug_buf = String::new();
    if let Ok(mut mon) = TcpStream::connect(("127.0.0.1", port)) {
        let _ = read_until_prompt(&mut mon, Duration::from_secs(2));
        registers = monitor_cmd(&mut mon, "info registers")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let base_raw = monitor_cmd(&mut mon, "x/gx 0xffffff8000e99b08");
        let ptr_raw = monitor_cmd(&mut mon, "x/gx 0xffffff8000e99b10");
        if let (Some(base), Some(ptr)) = (parse_monitor_hex64(&base_raw), parse_monitor_hex64(&ptr_raw)) {
            let n = ptr.saturating_sub(base).min(1500);
            if base != 0 && n > 0 {
                let raw_buf = monitor_cmd(&mut mon, &format!("x/{n}bc {base:#x}"));
                debug_buf = parse_monitor_chars(&raw_buf);
            }
        }
        let _ = monitor_cmd(&mut mon, "quit");
    }

    drop(guard);
    let duration = start.elapsed();
    let serial_log = std::fs::read_to_string(&serial_path).unwrap_or_default();
    if !debug_buf.is_empty() {
        let _ = std::fs::write(run_dir.join("debug_buf.txt"), &debug_buf);
    }
    let milestones = analyze_boot_output(&serial_log, &debug_buf);

    Ok(BootResult {
        duration,
        registers,
        debug_buf,
        serial_log,
        milestones,
    })
}
