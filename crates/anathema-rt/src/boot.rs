use alloc::vec::Vec;
use crate::dt;
use crate::macho::Image;
use crate::mem::{Ram, RAM_BASE};

pub const KERNEL_PA: u64 = RAM_BASE + 0x0300_4000;
pub const FB_PA: u64 = 0x6000_0000;

pub struct Loaded {
    pub entry_pa: u64,
    pub args_pa: u64,
    pub virt_base: u64,
}

fn align(v: u64, a: u64) -> u64 {
    (v + a - 1) & !(a - 1)
}

pub fn load_kernel(ram: &Ram, img: &Image, data: &[u8], dt_bytes: &[u8], cmdline: &str, ramdisk: Option<&[u8]>) -> Loaded {
    let virt_base = img.lo();
    let span = img.hi() - virt_base;
    for s in &img.segs {
        if s.vsize == 0 {
            continue;
        }
        let dst = KERNEL_PA + (s.vm - virt_base);
        let z = ram.bytes(dst, s.vsize as usize);
        z.fill(0);
        if s.fsize > 0 {
            z[..s.fsize as usize].copy_from_slice(&data[s.off as usize..(s.off + s.fsize) as usize]);
        }
    }
    let mut applied = 0usize;
    let mut bad = 0usize;
    for c in &img.chains {
        for (i, st) in c.starts.iter().enumerate() {
            if *st == 0xffff {
                continue;
            }
            let mut va = virt_base + c.seg_off + i as u64 * c.page_size + (*st as u64 & 0x7fff);
            loop {
                let pa = KERNEL_PA + (va - virt_base);
                let v = ram.read(pa, 8);
                let (next, new) = match c.format {
                    7 | 8 => ((v >> 51) & 0xfff, virt_base + (v & 0x3fff_ffff)),
                    f => panic!("unsupported chained pointer format {}", f),
                };
                if new < virt_base || new >= virt_base + span {
                    bad += 1;
                }
                ram.write(pa, 8, new);
                applied += 1;
                if next == 0 {
                    break;
                }
                va += next * 4;
            }
        }
    }
    println!("kernel: virt base {:#x}, span {:#x}, entry va {:#x}, {} fixups ({} out of range)", virt_base, span, img.entry, applied, bad);

    let mut root = dt::parse(dt_bytes);
    if let Some(mem) = root.child_mut("memory") {
        let mut v = Vec::new();
        v.extend_from_slice(&RAM_BASE.to_le_bytes());
        v.extend_from_slice(&ram.len.to_le_bytes());
        mem.set("reg", v);
    }
    if crate::sys::var("RT_KEEPGFX").is_err() {
        if let Some(io) = root.child_mut("arm-io") {
            io.children.retain(|c| {
                let n = c.name();
                n != "gfx" && n != "iosurface-mapper"
            });
        }
    }
    if let Some(cpus) = root.child_mut("cpus") {
        cpus.children.retain(|c| {
            let n = c.name();
            !(n.starts_with("cpu") && n[3..].parse::<u32>().map(|v| v > 0).unwrap_or(false))
        });
    }
    let mut seed = vec![0u8; 256];
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    for b in seed.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    if let Some(chosen) = root.child_mut("chosen") {
        chosen.set("random-seed", seed);
        let mut b = cmdline.as_bytes().to_vec();
        b.push(0);
        chosen.set("boot-args", b);
        chosen.set("kaslr-seed", vec![0u8; 8]);
        chosen.set("debug-enabled", 1u32.to_le_bytes().to_vec());
        chosen.set("development-cert", 1u32.to_le_bytes().to_vec());
        chosen.set("nvram-proxy-data", nvram_image(0x2000));
        chosen.set("nvram-bank-size", 0x2000u32.to_le_bytes().to_vec());
        chosen.set("nvram-bank-count", 2u32.to_le_bytes().to_vec());
        chosen.set("nvram-current-bank", 0u32.to_le_bytes().to_vec());
        chosen.set("dram-base", RAM_BASE.to_le_bytes().to_vec());
        chosen.set("dram-size", ram.len.to_le_bytes().to_vec());
    }
    if ramdisk.is_some() {
        if let Some(chosen) = root.child_mut("chosen") {
            if let Some(mm) = chosen.child_mut("memory-map") {
                mm.set("RAMDisk", vec![0u8; 16]);
            }
        }
    }
    let dt_probe = dt::serialize(&root);
    let dt_pa = align(KERNEL_PA + span, 0x4000);
    let args_pa = align(dt_pa + dt_probe.len() as u64, 0x4000);
    let mut top = align(args_pa + 0x4000, 0x4000);
    if let Some(rd) = ramdisk {
        let rd_pa = top;
        let mut v = Vec::new();
        v.extend_from_slice(&rd_pa.to_le_bytes());
        v.extend_from_slice(&(rd.len() as u64).to_le_bytes());
        if let Some(chosen) = root.child_mut("chosen") {
            if let Some(mm) = chosen.child_mut("memory-map") {
                mm.set("RAMDisk", v);
            }
        }
        ram.bytes(rd_pa, rd.len()).copy_from_slice(rd);
        top = align(rd_pa + rd.len() as u64, 0x4000);
        println!("ramdisk at {:#x}, {} bytes", rd_pa, rd.len());
    }
    let dt_final = dt::serialize(&root);
    ram.bytes(dt_pa, dt_final.len()).copy_from_slice(&dt_final);
    let args = ram.bytes(args_pa, 0x800);
    args.fill(0);
    let put16 = |a: &mut [u8], o: usize, v: u16| a[o..o + 2].copy_from_slice(&v.to_le_bytes());
    let put32 = |a: &mut [u8], o: usize, v: u32| a[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let put64 = |a: &mut [u8], o: usize, v: u64| a[o..o + 8].copy_from_slice(&v.to_le_bytes());
    put16(args, 0, 2);
    put16(args, 2, 2);
    put64(args, 8, virt_base);
    put64(args, 0x10, KERNEL_PA);
    put64(args, 0x18, ram.len - (KERNEL_PA - RAM_BASE));
    put64(args, 0x20, top);
    let video = crate::sys::var("RT_VIDEO").is_ok();
    put64(args, 0x28, if video { FB_PA } else { 0 });
    put64(args, 0x30, if video { crate::sys::var("RT_VDISP").ok().and_then(|v| v.parse().ok()).unwrap_or(1) } else { 0 });
    put64(args, 0x38, if video { 1920 * 4 } else { 0 });
    put64(args, 0x40, if video { 1920 } else { 0 });
    put64(args, 0x48, if video { 1080 } else { 0 });
    put64(args, 0x50, if video { 32 } else { 0 });
    put32(args, 0x58, 0);
    put64(args, 0x60, virt_base + (dt_pa - KERNEL_PA));
    put32(args, 0x68, dt_final.len() as u32);
    let cl = cmdline.as_bytes();
    args[0x6c..0x6c + cl.len().min(1023)].copy_from_slice(&cl[..cl.len().min(1023)]);
    put64(args, 0x470, 0);
    put64(args, 0x478, ram.len);
    println!("devicetree at {:#x} ({} bytes), boot args at {:#x}, top of kernel data {:#x}", dt_pa, dt_final.len(), args_pa, top);
    Loaded { entry_pa: KERNEL_PA + (img.entry - virt_base), args_pa, virt_base }
}

fn nvram_image(size: usize) -> Vec<u8> {
    fn cksum(h: &[u8]) -> u8 {
        let mut s: u32 = 0;
        for (i, b) in h.iter().take(16).enumerate() {
            if i != 1 {
                s += *b as u32;
            }
        }
        while s > 255 {
            s = (s >> 8) + (s & 0xff);
        }
        s as u8
    }
    fn part(img: &mut [u8], off: usize, sig: u8, len16: usize, name: &str) {
        img[off] = sig;
        img[off + 2] = (len16 & 0xff) as u8;
        img[off + 3] = (len16 >> 8) as u8;
        for (i, c) in name.bytes().enumerate().take(12) {
            img[off + 4 + i] = c;
        }
        img[off + 1] = cksum(&img[off..off + 16]);
    }
    let mut img = vec![0u8; size];
    let hdr = 32usize;
    let common = (size - hdr) / 2 / 16 * 16;
    let system = size - hdr - common;
    part(&mut img, 0, 0x5a, hdr / 16, "nvram");
    part(&mut img, hdr, 0x70, common / 16, "common");
    part(&mut img, hdr + common, 0x70, system / 16, "system");
    img[20..24].copy_from_slice(&1u32.to_le_bytes());
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for byte in &img[20..] {
        a = (a + *byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    let ad = (b << 16) | a;
    img[16..20].copy_from_slice(&ad.to_le_bytes());
    img
}
