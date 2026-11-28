use alloc::{string::{String, ToString}, vec::Vec};
use crate::mem::{Bus, Ram};
use crate::mmu::Mmu;
use anathema_xlat::decode::{decode, Addr, AtomOp, Ext, ExcKind, Insn, Mode, ZR};
use anathema_xlat::run::Ctx;
use anathema_xlat::trans::{sysreg_id, Fetch};
use anathema_xlat::fx::HashMap;

pub struct Machine {
    pub ram: Ram,
    pub bus: Bus,
    pub mmu: Mmu,
    pub sys: HashMap<u16, u64>,
    pub counts: HashMap<String, usize>,
    pub started: u64,
    pub events: usize,
    pub skip: u64,
    pub wsearch: u64,
    pub wsearch_n: usize,
    pub aes: crate::aesdev::AesState,
    pub excl: Option<u64>,
    pub rwatch: Vec<(u64, u64)>,
    pub rwatch_budget: usize,
    pub stop_idle: bool,
    pub wfe_count: u64,
    pub wfe_pc: u64,
    pub pci: crate::pci::Pci,
    pub max_events: usize,
    pub hist: Vec<(u64, u32)>,
    pub no_irq: bool,
    pub irq_taken: u64,
    pub tm: [u64; 4],
    pub irq_force: bool,
    pub trace_budget: usize,
    pub flush: bool,
    pub elog: bool,
    pub watch: u64,
    pub watch_pa: u64,
    pub watch_len: u64,
    pub watch_hits: u64,
    pub last_write: bool,
    pub pan: u64,
    pub spsel: u64,
    pub sp_el1: u64,
    pub say_exc: bool,
    pub exc_count: u64,
    pub last_exc: (u64, u64, u64),
    pub hit: bool,
    pub cur_pc: u64,
    pub from: usize,
    pub daif: u64,
    tlb: HashMap<u64, (u64, u8)>,
    pub el0: bool,
}

pub enum Step {
    Next,
    Stop(String),
}

pub fn sysname(id: u16) -> String {
    let op0 = 2 | ((id >> 14) & 1);
    format!("S{}_{}_C{}_C{}_{}", op0, (id >> 11) & 7, (id >> 7) & 15, (id >> 3) & 15, id & 7)
}

fn rd(ctx: &Ctx, r: u8) -> u64 {
    match r {
        31 => 0,
        32 => ctx.x[31],
        r => ctx.x[r as usize],
    }
}

fn wr(ctx: &mut Ctx, r: u8, v: u64) {
    match r {
        31 => {}
        32 => ctx.x[31] = v,
        r => ctx.x[r as usize] = v,
    }
}

fn extend(v: u64, e: Ext) -> u64 {
    match e {
        Ext::Uxtb => v & 0xff,
        Ext::Uxth => v & 0xffff,
        Ext::Uxtw => v & 0xffff_ffff,
        Ext::Uxtx | Ext::Sxtx => v,
        Ext::Sxtb => v as u8 as i8 as i64 as u64,
        Ext::Sxth => v as u16 as i16 as i64 as u64,
        Ext::Sxtw => v as u32 as i32 as i64 as u64,
    }
}

fn mask(size: usize) -> u64 {
    if size >= 8 {
        u64::MAX
    } else {
        (1u64 << (size * 8)) - 1
    }
}

fn sext_to(v: u64, size: usize, bits64: bool) -> u64 {
    let shift = 64 - size as u32 * 8;
    let s = ((v << shift) as i64 >> shift) as u64;
    if bits64 {
        s
    } else {
        s & 0xffff_ffff
    }
}

impl Machine {
    pub fn new(ram: Ram) -> Machine {
        let mut sys = HashMap::default();
        sys.insert(sysreg_id(3, 0, 4, 2, 2), 0x4);
        sys.insert(sysreg_id(3, 0, 0, 0, 0), 0x612f_0000);
        sys.insert(sysreg_id(3, 3, 0, 0, 1), 0x8444_c004);
        sys.insert(sysreg_id(3, 0, 0, 0, 5), 0x8000_0000);
        sys.insert(sysreg_id(3, 3, 14, 0, 0), 24_000_000);
        sys.insert(sysreg_id(3, 3, 0, 0, 7), 4);
        Machine {
            ram,
            bus: Bus::new(),
            mmu: Mmu::default(),
            sys,
            counts: HashMap::default(),
            started: crate::sys::nanos(),
            events: 0,
            skip: 0,
            wsearch: crate::sys::var("RT_WSEARCH").ok().and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0),
            wsearch_n: 0,
            aes: crate::aesdev::AesState::new(),
            excl: None,
            rwatch: crate::sys::var("RT_RDWATCH").map(|v| v.split(',').filter_map(|p| { let mut it = p.split(':'); let a = u64::from_str_radix(it.next()?, 16).ok()?; let l = u64::from_str_radix(it.next()?, 16).ok()?; Some((a, l)) }).collect()).unwrap_or_default(),
            rwatch_budget: 200,
            stop_idle: crate::sys::var("RT_STOPIDLE").is_ok(),
            wfe_count: 0,
            wfe_pc: 0,
            pci: crate::pci::Pci::new(),
            max_events: crate::sys::var("RT_MAXSTEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX),
            hist: vec![(0, 0); 4096],
            no_irq: crate::sys::var("RT_NOIRQ").is_ok(),
            irq_taken: 0,
            tm: [1, u64::MAX, 0, u64::MAX],
            irq_force: false,
            trace_budget: 400,
            flush: false,
            elog: crate::sys::var("RT_ELOG").is_ok(),
            last_write: false,
            pan: 0,
            spsel: 1,
            sp_el1: 0,
            say_exc: crate::sys::var("RT_EXC").is_ok(),
            exc_count: 0,
            last_exc: (0, 0, 0),
            watch_len: crate::sys::var("RT_WATCHLEN").ok().and_then(|v| v.parse().ok()).unwrap_or(1),
            watch_hits: 0,
            watch_pa: crate::sys::var("RT_WATCHPA").ok().and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0),
            watch: crate::sys::var("RT_WATCH").ok().and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0),
            cur_pc: 0,
            hit: false,
            from: crate::sys::var("RT_FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX),
            daif: 0x3c0,
            tlb: HashMap::default(),
            el0: false,
        }
    }

    pub fn translate(&mut self, va: u64) -> Result<u64, String> {
        self.translate_acc(va, 0)
    }

    pub fn translate_acc(&mut self, va: u64, acc: u8) -> Result<u64, String> {
        if !self.mmu.enabled() {
            return Ok(va);
        }
        let page = va & !0x3fff;
        let (pa, flags) = if let Some((pa, fl)) = self.tlb.get(&page) {
            (*pa | (va & 0x3fff), *fl)
        } else {
            match self.mmu.translate(&self.ram, va) {
                Ok((pa, fl)) => {
                    self.tlb.insert(page, (pa & !0x3fff, fl));
                    (pa, fl)
                }
                Err(f) => {
                    let name = if f.kind == 1 { "access flag fault" } else { "translation fault" };
                    return Err(format!("{} at {:#x} (level {})", name, f.va, f.level));
                }
            }
        };
        let denied = |flags: u8, el0: bool| (el0 && flags & 1 == 0) || (acc == 1 && flags & 2 != 0) || (acc == 2 && el0 && flags & 4 != 0);
        if denied(flags, self.el0) {
            self.tlb.remove(&page);
            match self.mmu.translate(&self.ram, va) {
                Ok((pa2, fl2)) => {
                    self.tlb.insert(page, (pa2 & !0x3fff, fl2));
                    if denied(fl2, self.el0) {
                        return Err(format!("permission fault at {:#x} (level {})", va, (fl2 >> 4) & 3));
                    }
                    return Ok(pa2);
                }
                Err(f) => {
                    let name = if f.kind == 1 { "access flag fault" } else { "translation fault" };
                    return Err(format!("{} at {:#x} (level {})", name, f.va, f.level));
                }
            }
        }
        Ok(pa)
    }

    pub fn flush_tlb(&mut self) {
        self.tlb.clear();
        self.flush = true;
    }

    pub fn load(&mut self, va: u64, size: usize) -> Result<u64, String> {
        self.last_write = false;
        if size == 16 {
            return Err("wide load".into());
        }
        let pa = self.translate_acc(va, 0)?;
        if self.rwatch_budget > 0 && !self.rwatch.is_empty() {
            for (a, l) in self.rwatch.clone() {
                if pa >= a && pa < a + l {
                    self.rwatch_budget -= 1;
                    println!("RDWATCH pa {:#x} va {:#x} size {} pc {:#x} event {}", pa, va, size, self.cur_pc, self.events);
                    break;
                }
            }
        }
        if self.ram.contains(pa, size as u64) {
            Ok(self.ram.read(pa, size))
        } else if pa < self.ram.base && self.pci.claims(pa) {
            if pa >= crate::pci::ECAM_BASE && pa < crate::pci::ECAM_BASE + crate::pci::ECAM_SIZE {
                Ok(self.pci.ecam_read(pa - crate::pci::ECAM_BASE, size))
            } else {
                let b = self.pci.bar_base();
                Ok(self.pci.bar_read(pa - b, size))
            }
        } else if pa < self.ram.base {
            Ok(self.bus.read(pa, size))
        } else {
            Err(format!("load from unmapped physical {:#x} (va {:#x})", pa, va))
        }
    }

    pub fn store(&mut self, va: u64, size: usize, v: u64) -> Result<(), String> {
        self.last_write = true;
        if self.watch != 0 && va <= self.watch && va + size as u64 > self.watch {
            self.hit = true;
            println!("WATCH write {:#x} size {} value {:#x} at pc {:#x} (event {})", va, size, v, self.cur_pc, self.events);
        }
        if self.wsearch != 0 && size == 8 && v == self.wsearch && self.wsearch_n < 40 {
            self.wsearch_n += 1;
            println!("WSEARCH store {:#x} to va {:#x} pc {:#x} event {}", v, va, self.cur_pc, self.events);
        }
        let pa = self.translate_acc(va, 1)?;
        if self.watch_pa != 0 && pa + size as u64 > self.watch_pa && pa < self.watch_pa + self.watch_len {
            self.watch_hits += 1;
            if self.watch_hits <= 12 {
                println!("WATCHPA write pa {:#x} va {:#x} size {} value {:#x} at pc {:#x} (event {})", pa, va, size, v, self.cur_pc, self.events);
            }
        }
        if self.ram.contains(pa, size as u64) {
            self.ram.write(pa, size, v);
            Ok(())
        } else if pa < self.ram.base && self.pci.claims(pa) {
            if pa >= crate::pci::ECAM_BASE && pa < crate::pci::ECAM_BASE + crate::pci::ECAM_SIZE {
                self.pci.ecam_write(pa - crate::pci::ECAM_BASE, size, v);
            } else {
                let b = self.pci.bar_base();
                self.pci.bar_write(pa - b, size, v, &self.ram, &mut self.bus.gic);
            }
            Ok(())
        } else if pa < self.ram.base {
            self.bus.write(pa, size, v);
            Ok(())
        } else {
            Err(format!("store to unmapped physical {:#x} (va {:#x})", pa, va))
        }
    }

    pub fn fetch_res(&mut self, pc: u64) -> Result<u32, String> {
        let pa = self.translate_acc(pc, 2)?;
        if self.ram.contains(pa, 4) {
            Ok(self.ram.read(pa, 4) as u32)
        } else {
            Err(format!("instruction fetch from unmapped physical {:#x}", pa))
        }
    }

    pub fn fetch_word(&mut self, pc: u64) -> Option<u32> {
        let pa = self.translate_acc(pc, 2).ok()?;
        if self.ram.contains(pa, 4) {
            Some(self.ram.read(pa, 4) as u32)
        } else {
            None
        }
    }

    fn count(&mut self, key: String) {
        *self.counts.entry(key).or_default() += 1;
    }

    fn ea(&self, ctx: &Ctx, pc: u64, rn: u8, a: &Addr) -> (u64, Option<u64>) {
        match *a {
            Addr::Lit { off } => (pc.wrapping_add(off as u64), None),
            Addr::Imm { off, mode } => {
                let base = rd(ctx, rn);
                match mode {
                    Mode::Offset => (base.wrapping_add(off as u64), None),
                    Mode::Pre => {
                        let e = base.wrapping_add(off as u64);
                        (e, Some(e))
                    }
                    Mode::Post => (base, Some(base.wrapping_add(off as u64))),
                }
            }
            Addr::Reg { rm, ext, amt } => {
                let off = extend(rd(ctx, rm), ext) << amt;
                (rd(ctx, rn).wrapping_add(off), None)
            }
        }
    }

    fn pair_ea(&self, ctx: &Ctx, rn: u8, off: i64, mode: Mode) -> (u64, Option<u64>) {
        let base = rd(ctx, rn);
        match mode {
            Mode::Offset => (base.wrapping_add(off as u64), None),
            Mode::Pre => {
                let e = base.wrapping_add(off as u64);
                (e, Some(e))
            }
            Mode::Post => (base, Some(base.wrapping_add(off as u64))),
        }
    }

    fn qload(&mut self, ctx: &mut Ctx, rt: u8, va: u64, size: usize) -> Result<(), String> {
        let mut buf = [0u8; 16];
        let mut done = 0;
        while done < size {
            let chunk = if size - done >= 8 { 8 } else { size - done };
            let v = self.load(va + done as u64, chunk)?;
            buf[done..done + chunk].copy_from_slice(&v.to_le_bytes()[..chunk]);
            done += chunk;
        }
        ctx.q[rt as usize] = buf;
        Ok(())
    }

    fn qstore(&mut self, ctx: &Ctx, rt: u8, va: u64, size: usize) -> Result<(), String> {
        let buf = ctx.q[rt as usize];
        let mut done = 0;
        while done < size {
            let chunk = if size - done >= 8 { 8 } else { size - done };
            let mut b = [0u8; 8];
            b[..chunk].copy_from_slice(&buf[done..done + chunk]);
            self.store(va + done as u64, chunk, u64::from_le_bytes(b))?;
            done += chunk;
        }
        Ok(())
    }

    pub fn step(&mut self, ctx: &mut Ctx, runner_flush: &mut bool) -> Step {
        let pc = ctx.pc;
        let w = match self.fetch_res(pc) {
            Ok(w) => w,
            Err(m) => {
                if let Some((va, fsc)) = parse_fault(&m) {
                    self.exc_count += 1;
                    if self.exc_count > 50_000_000 {
                        return Step::Stop(format!("too many exceptions, last instruction fetch fault at {:#x}", pc));
                    }
                    let ec = if self.el0 { 0x20u64 } else { 0x21 };
                    self.take_sync(ctx, pc, (ec << 26) | (1 << 25) | fsc, Some(va));
                    if self.say_exc {
                        println!("exc #{} instruction abort va {:#x} fsc {:#x} -> {:#x}", self.exc_count, va, fsc, ctx.pc);
                    }
                    return Step::Next;
                }
                return Step::Stop(format!("instruction fetch failed at {:#x}: {}", pc, m));
            }
        };
        self.events += 1;
        if self.events > self.max_events {
            return Step::Stop("step limit".to_string());
        }
        self.cur_pc = pc;
        self.bus.cur_pc = pc;
        let hi = self.events & 4095;
        self.hist[hi] = (pc, w);
        if (self.irq_force || self.events & 0x3f == 0) && !self.no_irq {
            self.irq_force = false;
            if let Some(fiq) = self.irq_poll() {
                let masked = if fiq { self.daif & 0x40 != 0 } else { self.daif & 0x80 != 0 };
                if !masked {
                    self.take_irq(ctx, pc, fiq);
                    return Step::Next;
                }
            }
        }
        let insn = decode(w);
        if self.elog {
            let mut fp = 0u64;
            for (i, v) in ctx.x.iter().enumerate() {
                fp ^= v.rotate_left(i as u32 * 2 + 1);
            }
            println!("E {} {:x} {:08x} {:016x} f{:x}", self.events, pc, w, fp, ctx.flags & 0x8c1);
        }
        let mut r = self.exec(ctx, pc, w, insn);
        if let Step::Stop(m) = &r {
            if let Some((va, fsc)) = parse_fault(m) {
                self.exc_count += 1;
                if self.exc_count > 50_000_000 {
                    return r;
                }
                let ec = if self.el0 { 0x24u64 } else { 0x25 };
                let esr = (ec << 26) | (1 << 25) | ((self.last_write as u64) << 6) | fsc;
                self.take_sync(ctx, pc, esr, Some(va));
                if self.say_exc {
                    println!("exc #{} data abort va {:#x} fsc {:#x} write {} at pc {:#x} -> {:#x}", self.exc_count, va, fsc, self.last_write, pc, ctx.pc);
                    if crate::sys::var("RT_EXCREGS").is_ok() {
                        println!("   x0 {:#x} x1 {:#x} x8 {:#x} x9 {:#x} x16 {:#x} x19 {:#x} x20 {:#x}", ctx.x[0], ctx.x[1], ctx.x[8], ctx.x[9], ctx.x[16], ctx.x[19], ctx.x[20]);
                    }
                }
                self.last_exc = (pc, va, self.exc_count);
                r = Step::Next;
            }
        }
        if self.bus.aes_kick {
            self.bus.aes_kick = false;
            let mut fifo = core::mem::take(&mut self.bus.aes_fifo);
            let mut irqs = self.bus.aes_irq_status;
            let mut info = self.bus.aes_flag_info;
            self.aes.process(&mut fifo, &self.ram, &mut irqs, &mut info);
            self.bus.aes_fifo = fifo;
            self.bus.aes_irq_status = irqs;
            self.bus.aes_flag_info = info;
            self.bus.gic.level[50] = irqs & self.bus.aes_irq_enable != 0;
            self.irq_force = true;
        }
        if self.bus.gfx_kick {
            self.bus.gfx_kick = false;
            let p = self.bus.reg(0x3020_1008);
            let a = self.bus.reg(0x3020_101c) << 14;
            let b = self.bus.reg(0x3020_1030) << 14;
            let c = self.bus.reg(0x3020_1004);
            println!("GFXKICK prod {:#x} reg1c-pa {:#x} reg30-pa {:#x} reg04 {:#x}", p, a, b, c);
            for base in [a + 0x4000, b + 0x4000, a, b] {
                if self.ram.contains(base, 0x80) {
                    let mut line = String::new();
                    for i in 0..32u64 {
                        line.push_str(&format!("{:08x} ", self.ram.read(base + i * 4, 4)));
                    }
                    println!("   {:#x}: {}", base, line);
                }
            }
        }
        if self.hit {
            self.hit = false;
            println!("  regs x0 {:#x} x1 {:#x} x2 {:#x} x3 {:#x} x29 {:#x} x30 {:#x} sp {:#x}", ctx.x[0], ctx.x[1], ctx.x[2], ctx.x[3], ctx.x[29], ctx.x[30], ctx.x[31]);
        }
        if self.flush {
            self.flush = false;
            *runner_flush = true;
        }
        r
    }

    fn exec(&mut self, ctx: &mut Ctx, pc: u64, w: u32, insn: Insn) -> Step {
        let say = self.events <= self.trace_budget || self.events >= self.from;
        macro_rules! tryv {
            ($e:expr) => {
                match $e {
                    Ok(v) => v,
                    Err(e) => return Step::Stop(format!("{} at pc {:#x}", e, pc)),
                }
            };
        }
        match insn {
            Insn::Ld { size, sext, rt, rn, addr } => {
                let (ea, wb) = self.ea(ctx, pc, rn, &addr);
                let n = 1usize << size;
                let mut v = tryv!(self.load(ea, n));
                if sext == 1 {
                    v = sext_to(v, n, false);
                } else if sext == 2 {
                    v = sext_to(v, n, true);
                }
                wr(ctx, rt, v);
                if let Some(b) = wb {
                    wr(ctx, rn, b);
                }
            }
            Insn::St { size, rt, rn, addr } => {
                let (ea, wb) = self.ea(ctx, pc, rn, &addr);
                let n = 1usize << size;
                tryv!(self.store(ea, n, rd(ctx, rt) & mask(n)));
                if let Some(b) = wb {
                    wr(ctx, rn, b);
                }
            }
            Insn::Ldp { sz, sx, rt, rt2, rn, off, mode } => {
                let (ea, wb) = self.pair_ea(ctx, rn, off, mode);
                let n = 1usize << sz;
                let a = tryv!(self.load(ea, n));
                let b = tryv!(self.load(ea + n as u64, n));
                let (a, b) = if sx { (sext_to(a, 4, true), sext_to(b, 4, true)) } else { (a, b) };
                wr(ctx, rt, a);
                wr(ctx, rt2, b);
                if let Some(x) = wb {
                    wr(ctx, rn, x);
                }
            }
            Insn::Stp { sz, rt, rt2, rn, off, mode } => {
                let (ea, wb) = self.pair_ea(ctx, rn, off, mode);
                let n = 1usize << sz;
                tryv!(self.store(ea, n, rd(ctx, rt) & mask(n)));
                tryv!(self.store(ea + n as u64, n, rd(ctx, rt2) & mask(n)));
                if let Some(x) = wb {
                    wr(ctx, rn, x);
                }
            }
            Insn::Ldex { size, rt, rt2, rn, pair } => {
                let ea = rd(ctx, rn);
                let n = 1usize << size;
                let a = tryv!(self.load(ea, n));
                wr(ctx, rt, a);
                if pair {
                    let b = tryv!(self.load(ea + n as u64, n));
                    wr(ctx, rt2, b);
                }
                self.excl = Some(ea);
            }
            Insn::Stex { size, rs, rt, rt2, rn, pair } => {
                let ea = rd(ctx, rn);
                let n = 1usize << size;
                if self.excl == Some(ea) {
                    tryv!(self.store(ea, n, rd(ctx, rt) & mask(n)));
                    if pair {
                        tryv!(self.store(ea + n as u64, n, rd(ctx, rt2) & mask(n)));
                    }
                    wr(ctx, rs, 0);
                } else {
                    wr(ctx, rs, 1);
                }
                self.excl = None;
            }
            Insn::Ldar { size, rt, rn } => {
                let n = 1usize << size;
                let v = tryv!(self.load(rd(ctx, rn), n));
                wr(ctx, rt, v);
            }
            Insn::Stlr { size, rt, rn } => {
                let n = 1usize << size;
                tryv!(self.store(rd(ctx, rn), n, rd(ctx, rt) & mask(n)));
            }
            Insn::Atomic { op, size, rs, rt, rn } => {
                let ea = rd(ctx, rn);
                let n = 1usize << size;
                let old = tryv!(self.load(ea, n));
                let s = rd(ctx, rs) & mask(n);
                let signed = |v: u64| sext_to(v, n, true) as i64;
                let new = match op {
                    AtomOp::Add => old.wrapping_add(s),
                    AtomOp::Clr => old & !s,
                    AtomOp::Eor => old ^ s,
                    AtomOp::Set => old | s,
                    AtomOp::Smax => {
                        if signed(old) >= signed(s) {
                            old
                        } else {
                            s
                        }
                    }
                    AtomOp::Smin => {
                        if signed(old) <= signed(s) {
                            old
                        } else {
                            s
                        }
                    }
                    AtomOp::Umax => old.max(s),
                    AtomOp::Umin => old.min(s),
                    AtomOp::Swp => s,
                };
                tryv!(self.store(ea, n, new & mask(n)));
                wr(ctx, rt, old);
            }
            Insn::Cas { size, rs, rt, rn } => {
                let ea = rd(ctx, rn);
                let n = 1usize << size;
                let old = tryv!(self.load(ea, n));
                if old == rd(ctx, rs) & mask(n) {
                    tryv!(self.store(ea, n, rd(ctx, rt) & mask(n)));
                }
                wr(ctx, rs, old);
            }
            Insn::VDupE { q, size, idx, rd: vd, rn: vn } => {
                let e = 1usize << size;
                let src = ctx.q[vn as usize];
                let mut o = [0u8; 16];
                let total = if q { 16 } else { 8 };
                let base = idx as usize * e;
                for i in 0..total / e {
                    o[i * e..(i + 1) * e].copy_from_slice(&src[base..base + e]);
                }
                ctx.q[vd as usize] = o;
            }
            Insn::Crc { c, sz, rd: d, rn, rm } => {
                let poly: u32 = if c { 0x82f6_3b78 } else { 0xedb8_8320 };
                let mut crc = rd(ctx, rn) as u32;
                let data = rd(ctx, rm);
                let nbytes = 1usize << sz;
                for i in 0..nbytes {
                    crc ^= ((data >> (8 * i)) & 0xff) as u32;
                    for _ in 0..8 {
                        crc = if crc & 1 != 0 { (crc >> 1) ^ poly } else { crc >> 1 };
                    }
                }
                wr(ctx, d, crc as u64);
            }
            Insn::Casp { size, rs, rt, rn } => {
                let ea = rd(ctx, rn);
                let n = 1usize << size;
                let o0 = tryv!(self.load(ea, n));
                let o1 = tryv!(self.load(ea + n as u64, n));
                if o0 == rd(ctx, rs) & mask(n) && o1 == rd(ctx, rs + 1) & mask(n) {
                    tryv!(self.store(ea, n, rd(ctx, rt) & mask(n)));
                    tryv!(self.store(ea + n as u64, n, rd(ctx, rt + 1) & mask(n)));
                }
                wr(ctx, rs, o0);
                wr(ctx, rs + 1, o1);
            }
            Insn::VLd { size, rt, rn, addr } => {
                let (ea, wb) = self.ea(ctx, pc, rn, &addr);
                tryv!(self.qload(ctx, rt, ea, 1usize << size));
                if let Some(b) = wb {
                    wr(ctx, rn, b);
                }
            }
            Insn::VSt { size, rt, rn, addr } => {
                let (ea, wb) = self.ea(ctx, pc, rn, &addr);
                tryv!(self.qstore(ctx, rt, ea, 1usize << size));
                if let Some(b) = wb {
                    wr(ctx, rn, b);
                }
            }
            Insn::VLdp { sz, rt, rt2, rn, off, mode } => {
                let (ea, wb) = self.pair_ea(ctx, rn, off, mode);
                let n = 1usize << sz;
                tryv!(self.qload(ctx, rt, ea, n));
                tryv!(self.qload(ctx, rt2, ea + n as u64, n));
                if let Some(x) = wb {
                    wr(ctx, rn, x);
                }
            }
            Insn::VStp { sz, rt, rt2, rn, off, mode } => {
                let (ea, wb) = self.pair_ea(ctx, rn, off, mode);
                let n = 1usize << sz;
                tryv!(self.qstore(ctx, rt, ea, n));
                tryv!(self.qstore(ctx, rt2, ea + n as u64, n));
                if let Some(x) = wb {
                    wr(ctx, rn, x);
                }
            }
            Insn::Mrs { rt, id } => {
                let v = self.read_sys(id);
                if rt != ZR {
                    wr(ctx, rt, v);
                }
                self.count(format!("mrs {}", sysname(id)));
                if say {
                    println!("#{} {:#x} mrs x{} <- {} = {:#x}", self.events, pc, rt, sysname(id), v);
                }
            }
            Insn::Msr { rt, id } => {
                let v = rd(ctx, rt);
                self.write_sys(id, v);
                self.count(format!("msr {}", sysname(id)));
                if say {
                    println!("#{} {:#x} msr {} <- x{} = {:#x}", self.events, pc, sysname(id), rt, v);
                }
            }
            Insn::MsrImm { field, imm } => {
                if field == 4 {
                    self.pan = imm as u64 & 1;
                } else if field == 5 {
                    let v = imm as u64 & 1;
                    if v != self.spsel {
                        if v == 0 {
                            self.sp_el1 = ctx.x[31];
                            ctx.x[31] = ctx.sys[3];
                        } else {
                            ctx.sys[3] = ctx.x[31];
                            ctx.x[31] = self.sp_el1;
                        }
                        self.spsel = v;
                    }
                } else if field == 30 {
                    self.daif |= (imm as u64 & 0xf) << 6;
                } else if field == 31 {
                    self.daif &= !((imm as u64 & 0xf) << 6);
                    self.irq_force = true;
                }
                self.count(format!("msr-imm {}", field));
                if say {
                    println!("#{} {:#x} msr-imm field {} imm {}", self.events, pc, field, imm);
                }
            }
            Insn::Sys { l, op1, crn, crm, op2, rt } => {
                self.count(format!("sys {} {} c{} c{} {}", if l { "sysl" } else { "sys" }, op1, crn, crm, op2));
                if crn == 8 {
                    self.flush_tlb();
                }
                if crn == 7 && crm == 4 && op1 == 3 && op2 == 1 && !l {
                    let va = (if rt == 31 { 0 } else { ctx.x[rt as usize] }) & !63u64;
                    for i in 0..8u64 {
                        tryv!(self.store(va + i * 8, 8, 0));
                    }
                }
                if crn == 7 && crm == 8 && !l {
                    let va = if rt == 31 { 0 } else { ctx.x[rt as usize] };
                    let par = if !self.mmu.enabled() { va & 0x0000_ffff_ffff_f000 } else {
                        match self.mmu.translate(&self.ram, va) {
                            Ok((pa, _)) => (pa & 0x0000_ffff_ffff_f000) | 0x800,
                            Err(_) => 1,
                        }
                    };
                    let par = if par & 1 == 0 { (par & !0xfff) | (par & 0x800) | (va & 0 ) } else { par };
                    self.sys.insert(sysreg_id(3, 0, 7, 4, 0), par);
                }
                if say {
                    println!("#{} {:#x} sys op1 {} c{} c{} op2 {} x{}", self.events, pc, op1, crn, crm, op2, rt);
                }
            }
            Insn::Exc { kind: ExcKind::Svc, imm } => {
                self.take_sync(ctx, pc + 4, (0x15u64 << 26) | (1 << 25) | (imm as u64 & 0xffff), None);
                if self.say_exc {
                    println!("svc #{:#x} x16 {:#x} x0 {:#x} from {} pc {:#x}", imm, ctx.x[16], ctx.x[0], if self.el0 { "el0" } else { "el1" }, pc);
                }
                return Step::Next;
            }
            Insn::Exc { kind: ExcKind::Hvc | ExcKind::Smc, imm } => {
                let f = ctx.x[0];
                let (ret, name) = match f {
                    0x8400_0000 => (0x0001_0001, "psci version"),
                    0x8400_0006 => (2, "psci migrate info"),
                    0xc400_0003 | 0x8400_0003 => (u64::MAX, "psci cpu on"),
                    0x8400_0008 => return Step::Stop("psci system off".into()),
                    0x8400_0009 => return Step::Stop("psci system reset".into()),
                    f if f & 0xffff_ff00 == 0xc100_0000 => (0, "vm hypervisor service call"),
                    _ => (u64::MAX, "unknown call"),
                };
                self.count(format!("hvc/smc {:#x} {}", f, name));
                if say || name == "unknown call" {
                    println!("#{} {:#x} {} #{} function {:#x} args {:#x} {:#x} {:#x} -> {} ({:#x})", self.events, pc, if matches!(insn, Insn::Exc { kind: ExcKind::Hvc, .. }) { "hvc" } else { "smc" }, imm, f, ctx.x[1], ctx.x[2], ctx.x[3], name, ret);
                }
                ctx.x[0] = ret;
            }
            Insn::Eret => {
                let spsr = ctx.sys[5];
                let elr = ctx.sys[4];
                let m = spsr & 0xf;
                if m != 5 && m != 4 && m != 0 {
                    return Step::Stop(format!("eret to unsupported mode {:#x} at {:#x}", spsr, pc));
                }
                if (m == 4 || m == 0) && self.spsel == 1 {
                    self.sp_el1 = ctx.x[31];
                    ctx.x[31] = ctx.sys[3];
                    self.spsel = 0;
                } else if m == 5 && self.spsel == 0 {
                    ctx.sys[3] = ctx.x[31];
                    ctx.x[31] = self.sp_el1;
                    self.spsel = 1;
                }
                self.el0 = m == 0;
                self.excl = None;
                self.daif = spsr & 0x3c0;
                self.pan = (spsr >> 22) & 1;
                self.irq_force = true;
                ctx.flags = anathema_xlat::run::nzcv_to_flags(spsr, ctx.flags);
                ctx.pc = elr;
                if self.say_exc {
                    println!("eret -> {:#x}", elr);
                }
                return Step::Next;
            }
            Insn::Hint { op } if op == 2 => {
                self.skip += 2400;
                if self.stop_idle {
                    if self.wfe_pc == pc {
                        self.wfe_count += 1;
                    } else {
                        self.wfe_pc = pc;
                        self.wfe_count = 0;
                    }
                    if self.wfe_count > 200_000 {
                        return Step::Stop(format!("idle wfe at {:#x}", pc));
                    }
                }
            }
            Insn::Hint { op } if op == 3 => {
                self.irq_poll();
                if !self.bus.gic.wake() {
                    let now = self.now();
                    let mut dl = u64::MAX;
                    if self.tm[0] & 3 == 1 {
                        dl = dl.min(self.tm[1]);
                    }
                    if self.tm[2] & 3 == 1 {
                        dl = dl.min(self.tm[3]);
                    }
                    if dl == u64::MAX {
                        return Step::Stop(format!("wfi with no wake source at {:#x}", pc));
                    }
                    if dl > now {
                        self.skip += dl - now;
                    }
                }
                self.irq_force = true;
            }
            Insn::Hint { .. } => {}
            Insn::Unsup(_) if crate::simd::fallback(ctx, w) => {}
            Insn::Unsup(_) if w & 0xbe00_0000 == 0x0c00_0000 => tryv!(self.struct_ldst(ctx, w)),
            _ if crate::simd::fallback(ctx, w) => {}
            _ => return Step::Stop(format!("unhandled instruction {:08x} ({}) at {:#x}", w, insn.name(), pc)),
        }
        ctx.pc = pc + 4;
        Step::Next
    }


    fn struct_ldst(&mut self, ctx: &mut Ctx, w: u32) -> Result<(), String> {
        let q = (w >> 30) & 1;
        let single = (w >> 24) & 1 == 1;
        let post = (w >> 23) & 1 == 1;
        let load = (w >> 22) & 1 == 1;
        let rm = ((w >> 16) & 31) as u8;
        let rn = ((w >> 5) & 31) as u8;
        let rt = (w & 31) as usize;
        let base_reg = if rn == 31 { 32 } else { rn };
        let base = rd(ctx, base_reg);
        let datasize = if q == 1 { 16usize } else { 8 };
        let mut addr = base;
        let mut total = 0usize;
        if !single {
            let opcode = (w >> 12) & 0xf;
            let size = ((w >> 10) & 3) as usize;
            let esize = 1usize << size;
            let (rpt, selem) = match opcode {
                0x0 => (1, 4),
                0x2 => (4, 1),
                0x4 => (1, 3),
                0x6 => (3, 1),
                0x7 => (1, 1),
                0x8 => (1, 2),
                0xa => (2, 1),
                _ => return Err(format!("unsupported structure load/store {:08x}", w)),
            };
            if esize == 8 && q == 0 && selem > 1 {
                return Err(format!("reserved structure load/store {:08x}", w));
            }
            let elems = datasize / esize;
            for r in 0..rpt {
                for e in 0..elems {
                    for sidx in 0..selem {
                        let reg = (rt + r + sidx) % 32;
                        if load {
                            let v = self.load(addr, esize)?;
                            if e == 0 && sidx == 0 || true {
                                let mut qv = if e == 0 { [0u8; 16] } else { ctx.q[reg] };
                                qv[e * esize..(e + 1) * esize].copy_from_slice(&v.to_le_bytes()[..esize]);
                                ctx.q[reg] = qv;
                            }
                        } else {
                            let mut b = [0u8; 8];
                            b[..esize].copy_from_slice(&ctx.q[reg][e * esize..(e + 1) * esize]);
                            self.store(addr, esize, u64::from_le_bytes(b))?;
                        }
                        addr += esize as u64;
                        total += esize;
                    }
                }
            }
        } else {
            let opcode = (w >> 13) & 7;
            let r = (w >> 21) & 1;
            let s_bit = (w >> 12) & 1;
            let size = ((w >> 10) & 3) as usize;
            let scale = opcode >> 1;
            let selem = (((opcode & 1) << 1) | r) as usize + 1;
            if scale == 3 {
                let esize = 1usize << size;
                for sidx in 0..selem {
                    let reg = (rt + sidx) % 32;
                    let v = self.load(addr, esize)?;
                    let mut qv = [0u8; 16];
                    for i in 0..datasize / esize {
                        qv[i * esize..(i + 1) * esize].copy_from_slice(&v.to_le_bytes()[..esize]);
                    }
                    ctx.q[reg] = qv;
                    addr += esize as u64;
                    total += esize;
                }
            } else {
                let (esize, index) = match scale {
                    0 => (1usize, ((q << 3) | (s_bit << 2) | size as u32) as usize),
                    1 => (2usize, ((q << 2) | (s_bit << 1) | (size as u32 >> 1)) as usize),
                    _ => {
                        if size & 1 == 0 {
                            (4usize, ((q << 1) | s_bit) as usize)
                        } else {
                            (8usize, q as usize)
                        }
                    }
                };
                for sidx in 0..selem {
                    let reg = (rt + sidx) % 32;
                    if load {
                        let v = self.load(addr, esize)?;
                        let mut qv = ctx.q[reg];
                        qv[index * esize..(index + 1) * esize].copy_from_slice(&v.to_le_bytes()[..esize]);
                        ctx.q[reg] = qv;
                    } else {
                        let mut b = [0u8; 8];
                        b[..esize].copy_from_slice(&ctx.q[reg][index * esize..(index + 1) * esize]);
                        self.store(addr, esize, u64::from_le_bytes(b))?;
                    }
                    addr += esize as u64;
                    total += esize;
                }
            }
        }
        if post {
            let off = if rm == 31 { total as u64 } else { rd(ctx, rm) };
            wr(ctx, base_reg, base.wrapping_add(off));
        }
        Ok(())
    }

    pub fn irq_poll(&mut self) -> Option<bool> {
        let now = self.now();
        self.bus.gic.level[27] = self.tm[0] & 3 == 1 && now >= self.tm[1];
        self.bus.gic.level[30] = self.tm[2] & 3 == 1 && now >= self.tm[3];
        self.bus.gic.deliverable()
    }

    fn take_sync(&mut self, ctx: &mut Ctx, elr: u64, esr: u64, far: Option<u64>) {
        self.enter_el1(ctx, elr, 0);
        ctx.sys[7] = esr;
        if let Some(f) = far {
            ctx.sys[6] = f;
        }
    }

    fn enter_el1(&mut self, ctx: &mut Ctx, elr: u64, kind_off: u64) {
        self.excl = None;
        let from_el0 = self.el0;
        let base = if from_el0 {
            0x400
        } else if self.spsel == 0 {
            0
        } else {
            0x200
        };
        let pan_bit = self.pan << 22;
        if self.mmu.sctlr & (1 << 23) == 0 {
            self.pan = 1;
        }
        let m = if from_el0 {
            0
        } else if self.spsel == 0 {
            4
        } else {
            5
        };
        let spsr = (anathema_xlat::run::flags_to_nzcv(ctx.flags) & 0xf000_0000) | self.daif | pan_bit | m;
        if self.spsel == 0 {
            ctx.sys[3] = ctx.x[31];
            ctx.x[31] = self.sp_el1;
            self.spsel = 1;
        }
        ctx.sys[4] = elr;
        ctx.sys[5] = spsr;
        self.daif = 0x3c0;
        self.el0 = false;
        ctx.pc = ctx.sys[8] + base + kind_off;
    }

    fn take_irq(&mut self, ctx: &mut Ctx, pc: u64, fiq: bool) {
        self.enter_el1(ctx, pc, if fiq { 0x100 } else { 0x80 });
        self.irq_taken += 1;
        if self.say_exc {
            println!("irq #{} {} at pc {:#x} -> {:#x}", self.irq_taken, if fiq { "fiq" } else { "irq" }, pc, ctx.pc);
        }
    }

    fn dev_read(&mut self, id: u16) -> Option<u64> {
        let now = self.now();
        let ctl = |c: u64, cv: u64| (c & 3) | if c & 3 == 1 && now >= cv { 4 } else { 0 };
        let s = |crn: u32, crm: u32, op2: u32| sysreg_id(3, 0, crn, crm, op2);
        let t = |crm: u32, op2: u32| sysreg_id(3, 3, 14, crm, op2);
        if id == t(3, 0) {
            return Some(self.tm[1].wrapping_sub(now) as u32 as u64);
        }
        if id == t(3, 1) {
            return Some(ctl(self.tm[0], self.tm[1]));
        }
        if id == t(3, 2) {
            return Some(self.tm[1]);
        }
        if id == t(2, 0) {
            return Some(self.tm[3].wrapping_sub(now) as u32 as u64);
        }
        if id == t(2, 1) {
            return Some(ctl(self.tm[2], self.tm[3]));
        }
        if id == t(2, 2) {
            return Some(self.tm[3]);
        }
        if id == s(12, 1, 0) {
            return Some(match self.irq_poll() {
                Some(true) => 1 << 6,
                Some(false) => 1 << 7,
                None => 0,
            });
        }
        let g = &mut self.bus.gic;
        if id == s(12, 12, 0) {
            self.irq_force = true;
            return Some(g.ack(true));
        }
        if id == s(12, 8, 0) {
            self.irq_force = true;
            return Some(g.ack(false));
        }
        if id == s(12, 12, 2) {
            return Some(g.hppir(true));
        }
        if id == s(12, 8, 2) {
            return Some(g.hppir(false));
        }
        if id == s(12, 11, 3) {
            return Some(g.rpr());
        }
        if id == s(4, 6, 0) {
            return Some(g.pmr);
        }
        if id == s(12, 12, 4) {
            return Some(g.ctlr);
        }
        if id == s(12, 12, 5) {
            return Some(g.sre);
        }
        if id == s(12, 12, 6) {
            return Some(g.igrpen1);
        }
        if id == s(12, 8, 6) {
            return Some(g.igrpen0);
        }
        None
    }

    fn dev_write(&mut self, id: u16, v: u64) -> bool {
        let now = self.now();
        let s = |crn: u32, crm: u32, op2: u32| sysreg_id(3, 0, crn, crm, op2);
        let t = |crm: u32, op2: u32| sysreg_id(3, 3, 14, crm, op2);
        if id == t(3, 0) {
            self.tm[1] = now.wrapping_add(v as u32 as i32 as i64 as u64);
            return true;
        }
        if id == t(3, 1) {
            self.tm[0] = v & 3;
            return true;
        }
        if id == t(3, 2) {
            self.tm[1] = v;
            return true;
        }
        if id == t(2, 0) {
            self.tm[3] = now.wrapping_add(v as u32 as i32 as i64 as u64);
            return true;
        }
        if id == t(2, 1) {
            self.tm[2] = v & 3;
            return true;
        }
        if id == t(2, 2) {
            self.tm[3] = v;
            return true;
        }
        let g = &mut self.bus.gic;
        self.irq_force = true;
        if id == s(12, 12, 1) || id == s(12, 8, 1) {
            g.eoi((v & 0xff_ffff) as usize);
            return true;
        }
        if id == s(12, 11, 1) {
            g.deactivate((v & 0xff_ffff) as usize);
            return true;
        }
        if id == s(12, 11, 5) || id == s(12, 11, 6) || id == s(12, 11, 7) {
            g.sgi(v);
            return true;
        }
        if id == s(4, 6, 0) {
            g.pmr = v & 0xff;
            return true;
        }
        if id == s(12, 12, 4) {
            g.ctlr = (v & 3) | 0x8f00;
            return true;
        }
        if id == s(12, 12, 5) {
            g.sre = 7;
            return true;
        }
        if id == s(12, 12, 6) {
            g.igrpen1 = v & 1;
            return true;
        }
        if id == s(12, 8, 6) {
            g.igrpen0 = v & 1;
            return true;
        }
        false
    }

    pub fn now(&self) -> u64 {
        1_000_000 + self.events as u64 * 24 + self.skip
    }

    fn read_sys(&mut self, id: u16) -> u64 {
        if let Some(v) = self.dev_read(id) {
            return v;
        }
        if id == sysreg_id(3, 3, 4, 2, 1) {
            return self.daif;
        }
        if id == sysreg_id(3, 3, 14, 0, 1) || id == sysreg_id(3, 3, 14, 0, 2) {
            if crate::sys::var("RT_REALTIME").is_ok() {
                return crate::sys::nanos().wrapping_sub(self.started) * 24 / 1000;
            }
            return self.now();
        }
        if id == sysreg_id(3, 0, 4, 2, 0) {
            return self.spsel;
        }
        if id == sysreg_id(3, 0, 4, 2, 3) {
            return self.pan << 22;
        }
        self.sys.get(&id).copied().unwrap_or(0)
    }

    fn write_sys(&mut self, id: u16, v: u64) {
        if self.dev_write(id, v) {
            return;
        }
        self.irq_force = true;
        self.sys.insert(id, v);
        if id == sysreg_id(3, 3, 4, 2, 1) {
            self.daif = v & 0x3c0;
        }
        if id == sysreg_id(3, 0, 4, 2, 3) {
            self.pan = (v >> 22) & 1;
        }
        if id == sysreg_id(3, 0, 1, 0, 0) {
            self.mmu.sctlr = v;
            self.flush_tlb();
        } else if id == sysreg_id(3, 0, 2, 0, 2) {
            self.mmu.tcr = v;
            self.flush_tlb();
        } else if id == sysreg_id(3, 0, 2, 0, 0) {
            self.mmu.ttbr0 = v;
            self.flush_tlb();
        } else if id == sysreg_id(3, 0, 2, 0, 1) {
            self.mmu.ttbr1 = v;
            self.flush_tlb();
        }
    }
}

pub struct Fetcher<'a> {
    pub m: core::cell::RefCell<&'a mut Machine>,
}

impl<'a> Fetch for Fetcher<'a> {
    fn word(&self, pc: u64) -> Option<u32> {
        if pc & 3 != 0 {
            return None;
        }
        self.m.borrow_mut().fetch_word(pc)
    }
}

fn parse_fault(msg: &str) -> Option<(u64, u64)> {
    let (base, rest) = if let Some(r) = msg.strip_prefix("translation fault at ") {
        (0x4u64, r)
    } else if let Some(r) = msg.strip_prefix("permission fault at ") {
        (0xc, r)
    } else if let Some(r) = msg.strip_prefix("access flag fault at ") {
        (0x8, r)
    } else {
        return None;
    };
    let va = rest.split_whitespace().next().and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0);
    let level = rest.split("(level ").nth(1).and_then(|v| v.split(')').next()).and_then(|v| v.parse::<u64>().ok()).unwrap_or(3);
    Some((va, base + level))
}
