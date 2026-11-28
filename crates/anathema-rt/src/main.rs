mod boot;
mod aesdev;
mod sys;
#[macro_use]
extern crate alloc;
mod fbcheck;
mod gic;
mod dt;
mod machine;
mod macho;
mod mem;
mod mmu;
mod pci;
mod simd;

use anathema_xlat::run::{Ctx, Runner, Stop};
use anathema_xlat::trans::{Config, MemModel};
use machine::{Fetcher, Machine, Step};
use mem::{Ram, RAM_BASE, RAM_SIZE};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

static mut CTX_PTR: *const Ctx = std::ptr::null();

#[repr(C)]
struct SigAction {
    handler: usize,
    mask: [u64; 16],
    flags: i32,
    restorer: usize,
}

extern "C" {
    fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
}

extern "C" fn on_segv(_sig: i32, info: *const u8, _uc: *mut u8) {
    unsafe {
        let addr = *(info.add(16) as *const u64);
        let pc = if CTX_PTR.is_null() { 0 } else { (*CTX_PTR).pc };
        eprintln!("host fault at {:#x} in block starting at guest pc {:#x}", addr, pc);
        std::process::exit(99);
    }
}

fn install_segv() {
    let act = SigAction { handler: on_segv as *const () as usize, mask: [0; 16], flags: 4, restorer: 0 };
    unsafe {
        sigaction(11, &act, std::ptr::null_mut());
        sigaction(7, &act, std::ptr::null_mut());
    }
}

struct FileDisk {
    f: std::fs::File,
    sectors: u64,
}

impl pci::Disk for FileDisk {
    fn sectors(&self) -> u64 {
        self.sectors
    }

    fn read(&mut self, sector: u64, out: &mut [u8]) -> bool {
        use std::io::{Read, Seek, SeekFrom};
        if self.f.seek(SeekFrom::Start(sector * 512)).is_err() {
            return false;
        }
        let mut done = 0;
        while done < out.len() {
            match self.f.read(&mut out[done..]) {
                Ok(0) => {
                    for b in out[done..].iter_mut() {
                        *b = 0;
                    }
                    break;
                }
                Ok(n) => done += n,
                Err(_) => return false,
            }
        }
        true
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("fbcheck") {
        let per: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
        fbcheck::run(0x1234_5678_9abc_def1, per);
        return;
    }
    if args.get(1).map(|s| s.as_str()) == Some("dt") {
        let d = std::fs::read(&args[2]).expect("read devicetree");
        let root = dt::parse(&d);
        let depth: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(2);
        let mut node = &root;
        if let Some(path) = args.get(4) {
            for part in path.split('/') {
                node = node.child(part).expect("no such node");
            }
        }
        dt::dump(node, 0, depth, &[]);
        return;
    }
    let kpath = args.get(1).cloned().unwrap_or_else(|| "kc-vm.macho".into());
    let dpath = args.get(2).cloned().unwrap_or_else(|| "dt-vm.bin".into());
    let trace: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(200);
    let cmdline = std::env::var("RT_BOOTARGS").unwrap_or_else(|_| "-v serial=3 debug=0x14e".into());
    let kdata = std::fs::read(&kpath).expect("read kernel");
    let img = macho::parse(&kdata);
    let dtb = std::fs::read(&dpath).expect("read devicetree");
    let ram = Ram::new(RAM_BASE, RAM_SIZE);
    let rdisk = std::env::var("RT_RAMDISK").ok().map(|p| std::fs::read(p).expect("read ramdisk"));
    let loaded = boot::load_kernel(&ram, &img, &kdata, &dtb, &cmdline, rdisk.as_deref());
    let mut m = Machine::new(ram);
    if let Ok(path) = std::env::var("RT_DISK") {
        let f = std::fs::File::open(&path).expect("open disk");
        let sectors = f.metadata().map(|m| m.len() / 512).unwrap_or(0);
        m.pci.disk = Some(pci::Overlay::new(Box::new(FileDisk { f, sectors })));
    }
    m.trace_budget = trace;
    install_segv();
    let mut ctx = Box::new(Ctx::default());
    ctx.pc = loaded.entry_pa;
    ctx.x[0] = loaded.args_pa;
    unsafe {
        CTX_PTR = &*ctx as *const Ctx;
    }
    println!("starting at physical {:#x}, boot args {:#x}", ctx.pc, ctx.x[0]);

    let mut runners: HashMap<u64, Runner> = HashMap::new();
    let mut blocks = 0u64;
    let mut last = (u64::MAX, 0usize);
    let limit: u64 = std::env::var("RT_BLOCKS").ok().and_then(|s| s.parse().ok()).unwrap_or(400_000_000);
    let bps: Option<Vec<u64>> = std::env::var("RT_BP").ok().map(|v| v.split(',').filter_map(|a| u64::from_str_radix(a.trim_start_matches("0x"), 16).ok()).collect());
    let ring_on = std::env::var("RT_RING").is_ok() || bps.is_some();
    let trace_on = ring_on || std::env::var("RT_TRACE").is_ok();
    let mut ring: VecDeque<u64> = VecDeque::new();
    let ring_max: usize = std::env::var("RT_RINGN").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
    let reason = loop {
        let key = ctx.pc >> 30;
        let runner = runners.entry(key).or_insert_with(|| {
            let mut cfg = Config::new(MemModel::Exit);
            cfg.indirect_flags_dead = false;
            cfg.pc_base = key << 30;
            if ring_on {
                cfg.max_insns = 1;
            }
            Runner::new(cfg)
        });
        if let Some(bp) = &bps {
            if bp.contains(&ctx.pc) {
                print!("BP {:#x}:", ctx.pc);
                for r in 0..31 {
                    print!(" x{}={:#x}", r, ctx.x[r]);
                }
                println!(" sp={:#x}", ctx.x[31]);
                if std::env::var("RT_BPSTK").is_ok() {
                    for i in 0..10u64 {
                        println!("   [sp+{:#x}] = {:#x}", i * 8, m.load(ctx.x[31] + i * 8, 8).unwrap_or(0));
                    }
                }
                if std::env::var("RT_BPRING").is_ok() {
                    for p in ring.iter().rev().take(60).collect::<Vec<_>>().into_iter().rev() {
                        print!("{:x} ", p & 0xfffffff);
                    }
                    println!();
                }
            }
        }
        if trace_on {
            ring.push_back(ctx.pc);
            if ring.len() > ring_max {
                ring.pop_front();
            }
        }
        let stop = {
            let f = Fetcher { m: RefCell::new(&mut m) };
            runner.run_until_stop(&mut ctx, &f, if ring_on { 1 } else { 4096 })
        };
        match stop {
            Stop::Budget => {
                blocks += if ring_on { 1 } else { 4096 };
                if blocks > limit {
                    break format!("block budget reached at pc {:#x}", ctx.pc);
                }
            }
            Stop::Refused(e) => {
                let mut flush = false;
                match m.step(&mut ctx, &mut flush) {
                    Step::Next => {}
                    Step::Stop(r) => break format!("{} (translator: {})", r, e),
                }
            }
            Stop::Interp(pc) => {
                if pc == last.0 {
                    last.1 += 1;
                    if last.1 > 200_000 {
                        break format!("spinning at {:#x}", pc);
                    }
                } else {
                    last = (pc, 0);
                }
                let mut flush = false;
                match m.step(&mut ctx, &mut flush) {
                    Step::Next => {}
                    Step::Stop(r) => break r,
                }
                if flush {
                    for (k, r) in runners.iter_mut() {
                        if *k < (1u64 << 33) {
                            r.reset_cache();
                        }
                    }
                }
            }
        }
    };
    println!();
    if trace_on {
        println!("last program counters:");
        for pc in &ring {
            let w = m.fetch_word(*pc).unwrap_or(0);
            println!("  {:#x}  {:08x}", pc, w);
        }
    }
    println!("stopped: {}", reason);
    if let Ok(spec) = std::env::var("RT_DUMPPA") {
        for part in spec.split(',') {
            let mut it = part.split(':');
            let pa = u64::from_str_radix(it.next().unwrap_or("0"), 16).unwrap_or(0);
            let len: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(64);
            println!("dump pa {:#x}:", pa);
            let mut o = 0;
            while o < len {
                if m.ram.contains(pa + o as u64, 4) {
                    print!("{:08x} ", m.ram.read(pa + o as u64, 4));
                }
                o += 4;
                if o % 32 == 0 {
                    println!();
                }
            }
            println!();
        }
    }
    if std::env::var("RT_MSGBUF").is_ok() {
        let mut pa = RAM_BASE;
        let end = RAM_BASE + RAM_SIZE;
        while pa < end {
            if m.ram.read(pa, 4) == 0x063061 {
                let size = m.ram.read(pa + 4, 4) as usize;
                let bufx = m.ram.read(pa + 8, 4) as usize;
                if size >= 0x1000 && size <= 0x100000 && bufx <= size && m.ram.contains(pa, 16 + size as u64) {
                    let b = m.ram.bytes(pa + 16, size);
                    let txt: Vec<u8> = b.iter().cloned().filter(|c| *c != 0).collect();
                    if txt.len() > 16 {
                        println!("--- msgbuf at {:#x} size {} bufx {} ---", pa, size, bufx);
                        println!("{}", String::from_utf8_lossy(&txt));
                        println!("--- end msgbuf ---");
                    }
                }
            }
            pa += 8;
        }
    }
    if std::env::var("RT_HIST").is_ok() {
        let n: usize = std::env::var("RT_HIST").ok().and_then(|v| v.parse().ok()).unwrap_or(100);
        println!("last steps:");
        for k in (0..n).rev() {
            let e = m.events.wrapping_sub(k) & 4095;
            let (pc, w) = m.hist[e];
            println!("  {:#x} {:08x}", pc, w);
        }
    }
    println!("interpreter steps {}, mmu {}", m.events, if m.mmu.enabled() { "on" } else { "off" });
    println!("pc {:#x}", ctx.pc);
    for r in 0..31 {
        print!("x{:<2} {:016x}  ", r, ctx.x[r]);
        if r % 4 == 3 {
            println!();
        }
    }
    println!("sp  {:016x}", ctx.x[31]);
    {
        let mut fp = ctx.x[29];
        println!("backtrace:");
        for _ in 0..14 {
            let lr = match m.load(fp + 8, 8) { Ok(v) => v, Err(_) => break };
            println!("  lr {:#x}", lr);
            if std::env::var("RT_FRAMES").is_ok() {
                let mut line = String::new();
                for i in 0..std::env::var("RT_WORDS").ok().and_then(|v| v.parse().ok()).unwrap_or(14u64) {
                    line.push_str(&format!("{:#x} ", m.load(fp.wrapping_sub(std::env::var("RT_BELOW").ok().and_then(|v| u64::from_str_radix(&v, 16).ok()).unwrap_or(0x70)) + i * 8, 8).unwrap_or(0)));
                }
                println!("    below fp: {}", line);
            }
            fp = match m.load(fp, 8) { Ok(v) => v, Err(_) => break };
            if fp == 0 { break }
        }
    }
    if let Ok(v) = std::env::var("RT_WALK") {
        if let Ok(va) = u64::from_str_radix(v.trim_start_matches("0x"), 16) {
            print!("{}", m.mmu.walk_debug(&m.ram, va));
        }
    }
    if std::env::var("RT_PAS").is_ok() {
        let mut a = ctx.x[31];
        for _ in 0..600 {
            let v = m.load(a, 8).unwrap_or(0);
            if (0x7000_0000..0x2_0000_0000).contains(&v) {
                println!("pa-like {:#x}: {:#x}", a, v);
            }
            a += 8;
        }
    }
    {
        println!("stack:");
        let mut a = ctx.x[31];
        for _ in 0..48 {
            let v = m.load(a, 8).unwrap_or(0);
            println!("  {:#x}: {:#x}", a, v);
            a += 8;
        }
    }
    if let Ok(list) = std::env::var("RT_PEEK") {
        for a in list.split(',') {
            if let Ok(va) = u64::from_str_radix(a.trim_start_matches("0x"), 16) {
                println!("peek {:#x} = {:?}", va, m.load(va, 8));
            }
        }
    }
    {
        let tp = ctx.sys[2];
        let a = m.load(tp + 408, 8).unwrap_or(0);
        let b = m.load(a + 40, 8).unwrap_or(0);
        println!("tpidr_el1 {:#x} [+408]={:#x} [that+40]={:#x} elr {:#x} esr {:#x} far {:#x} spsr {:#x} vbar {:#x}", tp, a, b, ctx.sys[4], ctx.sys[7], ctx.sys[6], ctx.sys[5], ctx.sys[8]);
    }
    if let Ok(path) = std::env::var("RT_FBDUMP") {
        let buf = m.bus.fb.clone();
        let _ = std::fs::write(path, buf);
    }
    println!("sctlr {:#x} tcr {:#x} ttbr0 {:#x} ttbr1 {:#x}", m.mmu.sctlr, m.mmu.tcr, m.mmu.ttbr0, m.mmu.ttbr1);
    match m.translate(ctx.pc) {
        Ok(pa) => {
            println!("pc translates to {:#x}", pa);
            if m.ram.contains(pa, 16) {
                println!("bytes there: {:08x} {:08x} {:08x} {:08x}", m.ram.read(pa, 4), m.ram.read(pa + 4, 4), m.ram.read(pa + 8, 4), m.ram.read(pa + 12, 4));
            }
        }
        Err(e) => println!("pc does not translate: {}", e),
    }
    let expect = boot::KERNEL_PA + (ctx.pc.wrapping_sub(loaded.virt_base));
    if m.ram.contains(expect, 16) {
        println!("linear expectation {:#x}: {:08x} {:08x} {:08x} {:08x}", expect, m.ram.read(expect, 4), m.ram.read(expect + 4, 4), m.ram.read(expect + 8, 4), m.ram.read(expect + 12, 4));
    }
    let mut top: Vec<_> = m.counts.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    println!("system access summary:");
    for (k, v) in top.iter().take(30) {
        println!("  {:8} {}", v, k);
    }
    if !m.bus.uart.is_empty() {
        println!("uart output ({} bytes) shown above", m.bus.uart.len());
    }
}
