use crate::gfx::*;
use crate::exfat;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use anathema_rt::mem::{Ram, RAM_BASE};
use anathema_rt::vm::Vm;
use core::ptr::NonNull;
use uefi::boot::{self, AllocateType, MemoryType};
use uefi::cstr16;
use uefi::fs::{FileSystem, PathBuf};
use uefi::proto::console::text::Key as Raw;
use uefi::system::with_stdin;

static mut LOG: Vec<String> = Vec::new();
static mut PARTIAL: String = String::new();

fn sink(s: &str) {
    unsafe {
        let partial = &mut *core::ptr::addr_of_mut!(PARTIAL);
        let log = &mut *core::ptr::addr_of_mut!(LOG);
        for ch in s.chars() {
            if ch == '\n' {
                log.push(core::mem::take(partial));
                if log.len() > 400 {
                    log.drain(0..200);
                }
            } else {
                partial.push(ch);
            }
        }
    }
}

fn alloc_exec(len: usize) -> *mut u8 {
    let pages = (len + 4095) / 4096;
    match boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_CODE, pages) {
        Ok(p) => p.as_ptr(),
        Err(_) => core::ptr::null_mut(),
    }
}

fn alloc_ram(want: usize) -> Option<(NonNull<u8>, usize)> {
    let mut size = want;
    while size >= 0x2000_0000 {
        if let Ok(p) = boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, size / 4096) {
            unsafe {
                core::ptr::write_bytes(p.as_ptr(), 0, size);
            }
            return Some((p, size));
        }
        size /= 2;
    }
    None
}

fn read_file(fs: &mut FileSystem, path: &uefi::CStr16) -> Result<Vec<u8>, String> {
    fs.read(PathBuf::from(path)).map_err(|e| format!("{}: {}", path, e))
}

fn escape_pressed() -> bool {
    let raw = with_stdin(|input| input.read_key());
    matches!(raw, Ok(Some(Raw::Special(code))) if code == uefi::proto::console::text::ScanCode::ESCAPE)
}

fn show(screen: &mut Screen, title: &str, lines: &[String], fb: Option<&[u8]>) {
    screen.clear();
    let s = screen.scale();
    let mut top = 0;
    if let Some(fb) = fb {
        let band = screen.h * 3 / 4;
        screen.blit_guest(fb, 1920, 1080, 0);
        top = band;
        screen.fill(0, top, screen.w, screen.h - top, BACKGROUND);
    }
    screen.text(8 * s, top + 4 * s, title, s, EMBER_SOFT);
    let mut y = top + 4 * s + 12 * s;
    let rows = (screen.h.saturating_sub(y)) / (9 * s);
    let start = lines.len().saturating_sub(rows);
    for line in &lines[start..] {
        screen.text(8 * s, y, line, s, TEXT);
        y += 9 * s;
    }
    screen.present();
}

pub fn autoboot() -> bool {
    match boot::get_image_file_system(boot::image_handle()) {
        Ok(p) => {
            let mut fs = FileSystem::new(p);
            fs.try_exists(PathBuf::from(cstr16!("\\anathema\\autoboot"))).unwrap_or(false)
        }
        Err(_) => false,
    }
}

pub fn run(screen: &mut Screen) -> String {
    sink("");
    anathema_rt::sys::set_sink(sink);
    anathema_rt::sys::set_var("RT_VIDEO", "1");
    anathema_xlat::run::ALLOC_EXEC.store(alloc_exec as usize, core::sync::atomic::Ordering::Relaxed);
    let proto = match boot::get_image_file_system(boot::image_handle()) {
        Ok(p) => p,
        Err(e) => return format!("no boot volume: {:?}", e.status()),
    };
    let mut fs = FileSystem::new(proto);
    let kernel = match read_file(&mut fs, cstr16!("\\anathema\\kernel.macho")) {
        Ok(k) => k,
        Err(e) => return format!("cannot read kernel: {}", e),
    };
    let dtb = match read_file(&mut fs, cstr16!("\\anathema\\dt.bin")) {
        Ok(k) => k,
        Err(e) => return format!("cannot read device tree: {}", e),
    };
    let cmdline = match read_file(&mut fs, cstr16!("\\anathema\\bootargs.txt")) {
        Ok(b) => String::from_utf8_lossy(&b).trim().into(),
        Err(_) => String::from("-v serial=3 debug=0x14e"),
    };
    let ramdisk = match exfat::read_file("anathema/ramdisk.bin") {
        Ok(r) => {
            sink(&format!("ramdisk {} bytes from the data partition\n", r.len()));
            Some(r)
        }
        Err(e) => {
            sink(&format!("no ramdisk: {}\n", e));
            None
        }
    };
    let cmdline = if ramdisk.is_some() && !cmdline.contains("rd=") { format!("{} rd=md0", cmdline) } else { cmdline };
    let (ptr, size) = match alloc_ram(0x8000_0000) {
        Some(x) => x,
        None => return String::from("not enough memory for the guest"),
    };
    let ram = unsafe { Ram::from_raw(RAM_BASE, size as u64, ptr.as_ptr()) };
    sink(&format!("guest memory {} MB, kernel {} bytes, device tree {} bytes", size >> 20, kernel.len(), dtb.len()));
    sink("
");
    let mut vm = Vm::new(ram, &kernel, &dtb, &cmdline, ramdisk.as_deref());
    drop(kernel);
    drop(dtb);
    drop(ramdisk);
    let mut frames = 0u32;
    let mut last_shown = 0usize;
    {
        let lines: Vec<String> = unsafe { (&*core::ptr::addr_of!(LOG)).clone() };
        show(screen, "macOS kernel starting  esc to stop", &lines, Some(&vm.m.bus.fb));
    }
    let mut reason = String::new();
    let mut spins = 0u64;
    loop {
        if let Some(r) = vm.run(40) {
            reason = r;
            break;
        }
        spins += 1;
        if vm.m.events >= last_shown + 2_000_000 {
            last_shown = vm.m.events;
            let lines: Vec<String> = unsafe { (&*core::ptr::addr_of!(LOG)).clone() };
            let title = format!("macOS kernel running  steps {}  esc to stop", vm.m.events);
            show(screen, &title, &lines, Some(&vm.m.bus.fb));
            frames += 1;
            if escape_pressed() {
                reason = String::from("stopped by user");
                break;
            }
        }
    }
    let _ = frames;
    let mut lines: Vec<String> = unsafe { (&*core::ptr::addr_of!(LOG)).clone() };
    lines.push(format!("stopped: {}", reason));
    lines.push(format!("steps {}  pc {:#x}", vm.m.events, vm.ctx.pc));
    let title = String::from("macOS kernel stopped  press any key");
    show(screen, &title, &lines, Some(&vm.m.bus.fb));
    let _ = crate::input::next();
    let mut full = String::new();
    for l in &lines {
        full.push_str(l);
        full.push_str("\r\n");
    }
    let _ = fs.write(PathBuf::from(cstr16!("\\anathema-macos.log")), full.as_bytes());
    format!("macOS stopped: {}", reason)
}
