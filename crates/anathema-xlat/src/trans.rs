use alloc::vec::Vec;
use crate::decode::*;
use crate::x86::*;

pub const T0: u8 = RAX;
pub const T1: u8 = RDX;
pub const T2: u8 = R11;
pub const CTX: u8 = R15;

pub const OFF_PC: i32 = 256;
pub const OFF_FLAGS: i32 = 264;
pub const OFF_EXVAL: i32 = 272;
pub const OFF_EXADDR: i32 = 280;
pub const OFF_LIMIT: i32 = 288;
pub const OFF_SYS: i32 = 296;
pub const SYS_SLOTS: usize = 32;
pub const OFF_Q: i32 = 296 + 8 * SYS_SLOTS as i32;
pub const CTX_SIZE: usize = OFF_Q as usize + 512;

pub fn sysreg_id(op0: u32, op1: u32, crn: u32, crm: u32, op2: u32) -> u16 {
    ((((op0 & 1) << 14) | (op1 << 11) | (crn << 7) | (crm << 3) | op2) & 0x7fff) as u16
}

pub fn sys_slot(id: u16) -> Option<usize> {
    const T: [(u32, u32, u32, u32, u32); 16] = [
        (3, 3, 13, 0, 2),
        (3, 3, 13, 0, 3),
        (3, 0, 13, 0, 4),
        (3, 0, 4, 1, 0),
        (3, 0, 4, 0, 1),
        (3, 0, 4, 0, 0),
        (3, 0, 6, 0, 0),
        (3, 0, 5, 2, 0),
        (3, 0, 12, 0, 0),
        (3, 0, 13, 0, 1),
        (3, 0, 1, 0, 2),
        (3, 3, 13, 0, 1),
        (3, 0, 10, 2, 0),
        (3, 0, 10, 3, 0),
        (3, 0, 14, 1, 0),
        (3, 0, 4, 0, 2),
    ];
    for (i, t) in T.iter().enumerate() {
        if sysreg_id(t.0, t.1, t.2, t.3, t.4) == id {
            return Some(i);
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemModel {
    Identity,
    Biased,
    Checked,
    Exit,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub pins: [u8; 33],
    pub vpins: [u8; 32],
    pub mem: MemModel,
    pub pc_base: u64,
    pub max_insns: usize,
    pub eflags_base: u32,
    pub bias: u64,
    pub indirect_flags_dead: bool,
}

pub const NOPIN: u8 = 0xff;
pub const NVPIN: usize = 13;
pub const DEFAULT_VORDER: [u8; NVPIN] = [0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20];

pub const PIN_REGS: [u8; 11] = [RBX, RBP, RSI, RDI, RCX, R8, R9, R10, R12, R13, R14];

pub const DEFAULT_ORDER: [u8; 20] =
    [0, 1, 2, 3, 4, 5, 6, 8, 19, 20, 21, 29, 30, 32, 7, 22, 23, 24, 25, 9];

impl Config {
    pub fn new(mem: MemModel) -> Config {
        Config::with_order(mem, &DEFAULT_ORDER)
    }

    pub fn with_order(mem: MemModel, order: &[u8]) -> Config {
        let mut pins = [NOPIN; 33];
        let mut regs: Vec<u8> = PIN_REGS.to_vec();
        if mem != MemModel::Biased {
        } else {
            regs.retain(|&r| r != R14);
        }
        for (g, h) in order.iter().zip(regs.iter()) {
            pins[*g as usize] = *h;
        }
        let mut vpins = [NOPIN; 32];
        for (i, v) in DEFAULT_VORDER.iter().enumerate() {
            vpins[*v as usize] = i as u8;
        }
        Config { pins, vpins, mem, pc_base: 0, max_insns: 128, eflags_base: 0x202, bias: 0, indirect_flags_dead: true }
    }

    pub fn set_vorder(&mut self, order: &[u8]) {
        self.vpins = [NOPIN; 32];
        for (i, v) in order.iter().take(NVPIN).enumerate() {
            self.vpins[*v as usize] = i as u8;
        }
    }

    pub fn vpinned(&self) -> Vec<(u8, u8)> {
        (0..32u8).filter(|&v| self.vpins[v as usize] != NOPIN).map(|v| (v, self.vpins[v as usize])).collect()
    }

    pub fn pinned(&self) -> Vec<(u8, u8)> {
        (0..33u8).filter(|&g| self.pins[g as usize] != NOPIN).map(|g| (g, self.pins[g as usize])).collect()
    }
}

pub trait Fetch {
    fn word(&self, pc: u64) -> Option<u32>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shared {
    Dispatch,
    Indirect,
    Interp,
}

#[derive(Clone, Copy, Debug)]
pub enum Fix {
    Cold { at: u32, to: u32 },
    Hot { at: u32, to: u32 },
    Shared { cold: bool, at: u32, id: Shared },
}

#[derive(Clone, Copy, Debug)]
pub struct Exit {
    pub hot_at: u32,
    pub target: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub hot: Vec<u8>,
    pub cold: Vec<u8>,
    pub fixes: Vec<Fix>,
    pub exits: Vec<Exit>,
    pub meta: Vec<(u32, u16)>,
    pub n_insns: usize,
    pub n_mem: usize,
    pub end_pc: u64,
    pub refused: Option<(u32, &'static str)>,
    pub hooked: usize,
}

impl Trace {
    pub fn total(&self) -> usize {
        self.hot.len() + self.cold.len()
    }

    pub fn layout(&self, hot_base: usize, cold_base: usize, shared: &dyn Fn(Shared) -> usize) -> (Vec<u8>, Vec<u8>) {
        let mut h = self.hot.clone();
        let mut c = self.cold.clone();
        for f in &self.fixes {
            match *f {
                Fix::Cold { at, to } => {
                    let rel = (cold_base + to as usize) as i64 - (hot_base + at as usize + 4) as i64;
                    h[at as usize..at as usize + 4].copy_from_slice(&(rel as i32).to_le_bytes());
                }
                Fix::Hot { at, to } => {
                    let rel = (hot_base + to as usize) as i64 - (cold_base + at as usize + 4) as i64;
                    c[at as usize..at as usize + 4].copy_from_slice(&(rel as i32).to_le_bytes());
                }
                Fix::Shared { cold, at, id } => {
                    let (buf, base) = if cold { (&mut c, cold_base) } else { (&mut h, hot_base) };
                    let rel = shared(id) as i64 - (base + at as usize + 4) as i64;
                    buf[at as usize..at as usize + 4].copy_from_slice(&(rel as i32).to_le_bytes());
                }
            }
        }
        (h, c)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fl {
    Inv,
    Add,
    Sub,
}

#[derive(Clone, Copy, Debug)]
enum B {
    G(u8),
    I(i64),
    H(u8),
}

#[derive(Clone, Copy, Debug)]
enum Bb {
    Reg(u8),
    Slot(Mem),
    Imm(i32),
}

fn fits32(v: i64) -> bool {
    v >= i32::MIN as i64 && v <= i32::MAX as i64
}

fn slot_idx(g: u8) -> i32 {
    if g == SP {
        31
    } else {
        g as i32
    }
}

pub fn gslot(g: u8) -> Mem {
    Mem::b(CTX, slot_idx(g) * 8 - 128)
}

pub fn ctxm(off: i32) -> Mem {
    Mem::b(CTX, off - 128)
}

fn flag_rw(i: &Insn) -> (bool, bool) {
    match *i {
        Insn::Bcond { cond, .. } => (cond < 14, false),
        Insn::Csel { cond, .. } | Insn::Fcsel { cond, .. } => (cond < 14, false),
        Insn::Fcmp { .. } => (false, true),
        Insn::Ccmp { cond, .. } => (cond < 14, true),
        Insn::AddSubC { set, .. } => (true, set),
        Insn::AddSubImm { set, .. } | Insn::AddSubSh { set, .. } | Insn::AddSubExt { set, .. } => (false, set),
        Insn::LogImm { op, .. } | Insn::LogSh { op, .. } => (false, op == LogOp::Ands),
        _ => (false, false),
    }
}

pub fn is_mem(i: &Insn) -> bool {
    matches!(
        i,
        Insn::Ld { .. }
            | Insn::St { .. }
            | Insn::Ldp { .. }
            | Insn::Stp { .. }
            | Insn::Ldex { .. }
            | Insn::Stex { .. }
            | Insn::Ldar { .. }
            | Insn::Stlr { .. }
            | Insn::Atomic { .. }
            | Insn::Cas { .. }
            | Insn::Casp { .. }
            | Insn::VLd { .. }
            | Insn::VSt { .. }
            | Insn::VLdp { .. }
            | Insn::VStp { .. }
    )
}

pub fn needs_interp(i: &Insn) -> bool {
    is_interp(i, true)
}

fn is_interp(i: &Insn, cfg_sys: bool) -> bool {
    let _ = cfg_sys;
    match *i {
        Insn::Unsup(_) => true,
        Insn::Eret | Insn::Exc { .. } | Insn::MsrImm { .. } | Insn::Sys { .. } => true,
        Insn::Mrs { id, .. } | Insn::Msr { id, .. } => sys_slot(id).is_none(),
        Insn::Hint { op } => matches!(op, 2 | 3 | 4 | 5),
        Insn::Crc { c, .. } => !c,
        Insn::VDupE { .. } => true,
        Insn::Ldex { pair: true, .. } | Insn::Stex { pair: true, .. } => true,
        Insn::Casp { .. } => true,
        Insn::Atomic { op, size, rs, .. } => {
            matches!(op, AtomOp::Smax | AtomOp::Smin | AtomOp::Umax | AtomOp::Umin) && (size < 2 || rs == ZR)
        }
        _ => false,
    }
}

pub fn flags_live_at(f: &dyn Fetch, mut pc: u64, depth: u32) -> bool {
    for _ in 0..8 {
        let w = match f.word(pc) {
            Some(w) => w,
            None => return true,
        };
        let i = decode(w);
        let (r, wr) = flag_rw(&i);
        if r {
            return true;
        }
        if wr {
            return false;
        }
        match i {
            Insn::Bl { .. } | Insn::Ret { .. } | Insn::Br { .. } | Insn::Blr { .. } => return false,
            Insn::B { off } => {
                if depth == 0 {
                    return true;
                }
                return flags_live_at(f, pc.wrapping_add(off as u64), depth - 1);
            }
            Insn::Bcond { .. } | Insn::Cbz { .. } | Insn::Tbz { .. } => return true,
            ref x if is_interp(x, false) => return true,
            _ => {}
        }
        pc = pc.wrapping_add(4);
    }
    true
}

struct Tx<'a> {
    cfg: &'a Config,
    a: Asm,
    fl: Fl,
    saved: bool,
    la: bool,
    fixes: Vec<Fix>,
    exits: Vec<Exit>,
    meta: Vec<(u32, u16)>,
    idx: usize,
    n_mem: usize,
    pc: u64,
    zs: u32,
    zd: u32,
}

pub fn translate(cfg: &Config, f: &dyn Fetch, start: u64) -> Trace {
    let mut insns: Vec<Insn> = Vec::new();
    let mut pcs: Vec<u64> = Vec::new();
    let mut pc = start;
    let mut stop_pc = None;
    let mut refused = None;
    let mut hooked = 0;
    loop {
        if insns.len() >= cfg.max_insns {
            stop_pc = Some(pc);
            break;
        }
        let w = match f.word(pc) {
            Some(w) => w,
            None => {
                stop_pc = Some(pc);
                break;
            }
        };
        let i = decode(w);
        if is_interp(&i, true) || (cfg.mem == MemModel::Exit && is_mem(&i)) {
            if insns.is_empty() {
                if let Insn::Unsup(n) = i {
                    refused = Some((w, n));
                } else {
                    hooked = 1;
                }
            }
            stop_pc = Some(pc);
            break;
        }
        insns.push(i);
        pcs.push(pc);
        pc = pc.wrapping_add(4);
        if matches!(
            i,
            Insn::B { .. } | Insn::Bl { .. } | Insn::Br { .. } | Insn::Blr { .. } | Insn::Ret { .. }
        ) {
            break;
        }
    }
    let n = insns.len();
    let mut taken_live = vec![false; n];
    for k in 0..n {
        let t = match insns[k] {
            Insn::Bcond { off, .. } | Insn::Cbz { off, .. } | Insn::Tbz { off, .. } => Some(off),
            _ => None,
        };
        if let Some(off) = t {
            taken_live[k] = flags_live_at(f, pcs[k].wrapping_add(off as u64), 2);
        }
    }
    let end_live = match insns.last() {
        Some(Insn::B { off }) => flags_live_at(f, pcs[n - 1].wrapping_add(*off as u64), 2),
        Some(Insn::Bl { .. }) | Some(Insn::Ret { .. }) | Some(Insn::Br { .. }) | Some(Insn::Blr { .. }) => {
            !cfg.indirect_flags_dead && !matches!(insns.last(), Some(Insn::Bl { .. }))
        }
        _ => match stop_pc {
            Some(p) => flags_live_at(f, p, 2),
            None => true,
        },
    };
    let mut la = vec![false; n];
    let mut live = end_live;
    for k in (0..n).rev() {
        let (r, w) = flag_rw(&insns[k]);
        let mut after = live;
        if matches!(insns[k], Insn::Bcond { .. } | Insn::Cbz { .. } | Insn::Tbz { .. }) {
            after |= taken_live[k];
        }
        la[k] = after;
        live = r || (after && !w);
    }
    let mut t = Tx {
        cfg,
        a: Asm::new(),
        fl: Fl::Inv,
        saved: true,
        la: false,
        fixes: Vec::new(),
        exits: Vec::new(),
        meta: Vec::new(),
        idx: 0,
        n_mem: 0,
        pc: 0,
        zs: 0,
        zd: 0,
    };
    let mut k = 0;
    let mut ended = false;
    while k < n {
        t.idx = k;
        t.la = la[k];
        t.pc = pcs[k];
        let used = t.insn(&insns[k..], pcs[k]);
        if t.ends(&insns[k]) {
            ended = true;
        }
        k += used;
    }
    if !ended {
        if let Some(p) = stop_pc {
            let lv = if n == 0 { true } else { flags_live_at(f, p, 2) };
            t.exit_direct(p, None, lv);
        }
    }
    Trace {
        hot: t.a.hot,
        cold: t.a.cold,
        fixes: t.fixes,
        exits: t.exits,
        meta: t.meta,
        n_insns: n,
        n_mem: t.n_mem,
        end_pc: stop_pc.unwrap_or(pc),
        refused,
        hooked,
    }
}

impl<'a> Tx<'a> {
    fn ends(&self, i: &Insn) -> bool {
        matches!(i, Insn::B { .. } | Insn::Bl { .. } | Insn::Br { .. } | Insn::Blr { .. } | Insn::Ret { .. })
    }

    fn pin(&self, g: u8) -> Option<u8> {
        if g == ZR {
            None
        } else {
            let p = self.cfg.pins[g as usize];
            if p == NOPIN {
                None
            } else {
                Some(p)
            }
        }
    }

    fn rm_of(&self, g: u8) -> Rm {
        match self.pin(g) {
            Some(h) => Rm::R(h),
            None => Rm::M(gslot(g)),
        }
    }

    fn load_g(&mut self, h: u8, g: u8) {
        if g == ZR {
            self.a.mov_ri(h, 0);
        } else {
            match self.pin(g) {
                Some(p) => {
                    if p != h {
                        self.a.mov_rr(true, h, p)
                    }
                }
                None => self.a.op(0, true, &[0x8b], h, Rm::M(gslot(g)), false),
            }
        }
    }

    fn src(&mut self, g: u8, tmp: u8) -> u8 {
        match self.pin(g) {
            Some(p) => p,
            None => {
                self.load_g(tmp, g);
                tmp
            }
        }
    }

    fn dst(&self, g: u8, tmp: u8) -> u8 {
        self.pin(g).unwrap_or(tmp)
    }

    fn commit(&mut self, g: u8, r: u8) {
        if g != ZR && self.pin(g).is_none() {
            self.a.store(3, gslot(g), r);
        }
    }

    fn set_flags(&mut self, style: Fl) {
        self.fl = style;
        self.saved = false;
    }

    fn spill(&mut self) {
        if self.fl == Fl::Inv || self.saved {
            return;
        }
        if self.fl == Fl::Add {
            self.a.cmc();
            self.fl = Fl::Sub;
        }
        self.a.pushfq();
        self.a.pop_m(ctxm(OFF_FLAGS));
        self.saved = true;
    }

    fn clobber(&mut self) {
        if self.fl != Fl::Inv {
            if self.la {
                self.spill();
            }
            self.fl = Fl::Inv;
        }
    }

    fn need_flags(&mut self) {
        if self.fl == Fl::Inv {
            self.a.push_m(ctxm(OFF_FLAGS));
            self.a.popfq();
            self.fl = Fl::Sub;
            self.saved = true;
        }
    }

    fn cc_for(&mut self, cond: u8) -> u8 {
        let b = cond >> 1;
        if b == 4 && self.fl == Fl::Add {
            self.a.cmc();
            self.fl = Fl::Sub;
        }
        let base: u8 = match b {
            0 => CC_E,
            1 => CC_AE,
            2 => CC_S,
            3 => CC_O,
            4 => CC_A,
            5 => CC_GE,
            _ => CC_G,
        };
        let mut inv = cond & 1;
        if b == 1 && self.fl == Fl::Add {
            inv ^= 1;
        }
        base ^ inv
    }

    fn pcimm(&self, pc: u64) -> (bool, u64) {
        let d = pc.wrapping_sub(self.cfg.pc_base);
        if d <= u32::MAX as u64 {
            (true, d)
        } else {
            (false, pc)
        }
    }

    fn emit_pc_load(&mut self, pc: u64) {
        let (small, v) = self.pcimm(pc);
        if small {
            self.a.mov_ri(RAX, v);
        } else {
            self.a.mov_ri64(RAX, v.wrapping_sub(self.cfg.pc_base));
        }
    }

    fn stub(&mut self, target: u64, shared: Shared, flags_live: bool) -> u32 {
        self.a.in_cold = true;
        let off = self.a.cold.len() as u32;
        if flags_live && self.fl != Fl::Inv && !self.saved {
            if self.fl == Fl::Add {
                self.a.cmc();
            }
            self.a.pushfq();
            self.a.pop_m(ctxm(OFF_FLAGS));
        }
        self.emit_pc_load(target);
        let at = self.a.jmp32() as u32;
        self.fixes.push(Fix::Shared { cold: true, at, id: shared });
        self.a.in_cold = false;
        off
    }

    fn exit_direct(&mut self, target: u64, cc: Option<u8>, flags_live: bool) {
        let s = self.stub(target, Shared::Dispatch, flags_live);
        let at = match cc {
            Some(c) => self.a.jcc32(c),
            None => self.a.jmp32(),
        } as u32;
        self.fixes.push(Fix::Cold { at, to: s });
        self.exits.push(Exit { hot_at: at, target });
    }

    fn mark(&mut self) {
        let p = self.a.hot.len() as u32;
        self.meta.push((p, self.idx as u16));
        self.n_mem += 1;
    }

    fn insn(&mut self, rest: &[Insn], pc: u64) -> usize {
        let i = rest[0];
        match i {
            Insn::Adr { rd, imm } => {
                self.mov_imm(rd, pc.wrapping_add(imm as u64), true);
            }
            Insn::Adrp { rd, imm } => {
                let mut v = (pc & !0xfff).wrapping_add(imm as u64);
                let mut used = 1;
                if let Some(Insn::AddSubImm { sf: true, sub: false, set: false, rd: rd2, rn, imm }) = rest.get(1) {
                    if *rd2 == rd && *rn == rd && rd != ZR && rd != SP {
                        v = v.wrapping_add(*imm as u64);
                        used = 2;
                    }
                }
                self.mov_imm(rd, v, true);
                return used;
            }
            Insn::MovW { sf, kind, rd, imm16, hw } => {
                let sh = 16 * hw as u32;
                let mut v: u64 = (imm16 as u64) << sh;
                match kind {
                    0 => {
                        v = !v;
                        if !sf {
                            v &= 0xffff_ffff;
                        }
                    }
                    2 => {}
                    _ => {
                        self.movk(sf, rd, imm16, hw);
                        return 1;
                    }
                }
                let mut used = 1;
                while let Some(Insn::MovW { sf: s2, kind: 3, rd: r2, imm16: m, hw: h }) = rest.get(used) {
                    if *r2 != rd || *s2 != sf {
                        break;
                    }
                    let s = 16 * *h as u32;
                    v = (v & !(0xffffu64 << s)) | ((*m as u64) << s);
                    used += 1;
                }
                self.mov_imm(rd, v, sf);
                return used;
            }
            Insn::AddSubImm { sf, sub, set, rd, rn, imm } => {
                let op = if sub { Alu::Sub } else { Alu::Add };
                self.alu3(op, sf, rd, rn, B::I(imm as i64), set);
            }
            Insn::AddSubSh { sf, sub, set, rd, rn, rm, sh, amt } => {
                self.addsub_sh(sf, sub, set, rd, rn, rm, sh, amt);
            }
            Insn::AddSubExt { sf, sub, set, rd, rn, rm, ext, amt } => {
                self.addsub_ext(sf, sub, set, rd, rn, rm, ext, amt);
            }
            Insn::AddSubC { sf, sub, set, rd, rn, rm } => self.addsubc(sf, sub, set, rd, rn, rm),
            Insn::LogImm { sf, op, rd, rn, imm } => {
                let v = if sf { imm as i64 } else { imm as u32 as i32 as i64 };
                let alu = match op {
                    LogOp::And | LogOp::Ands => Alu::And,
                    LogOp::Orr => Alu::Or,
                    LogOp::Eor => Alu::Xor,
                };
                self.logic_imm(sf, alu, op == LogOp::Ands, rd, rn, v, imm);
            }
            Insn::LogSh { sf, op, inv, rd, rn, rm, sh, amt } => self.logsh(sf, op, inv, rd, rn, rm, sh, amt),
            Insn::Bfm { sf, op, rd, rn, immr, imms } => self.bfm(sf, op, rd, rn, immr as u32, imms as u32),
            Insn::Extr { sf, rd, rn, rm, lsb } => self.extr(sf, rd, rn, rm, lsb),
            Insn::ShV { sf, sh, rd, rn, rm } => self.shv(sf, sh, rd, rn, rm),
            Insn::Madd { sf, sub, rd, rn, rm, ra } => self.madd(sf, sub, rd, rn, rm, ra),
            Insn::Mull { signed, sub, rd, rn, rm, ra } => self.mull(signed, sub, rd, rn, rm, ra),
            Insn::Mulh { signed, rd, rn, rm } => self.mulh(signed, rd, rn, rm),
            Insn::Div { sf, signed, rd, rn, rm } => self.div(sf, signed, rd, rn, rm),
            Insn::Csel { sf, op, rd, rn, rm, cond } => self.csel(sf, op, rd, rn, rm, cond),
            Insn::Ccmp { sf, neg, imm, rn, rm, nzcv, cond } => self.ccmp(sf, neg, imm, rn, rm, nzcv, cond),
            Insn::Clz { sf, rd, rn } => {
                if rd != ZR {
                    self.clobber();
                    let s = self.src(rn, T0);
                    let d = self.dst(rd, T0);
                    self.a.lzcnt(sf, d, s);
                    self.commit(rd, d);
                }
            }
            Insn::Cls { sf, rd, rn } => {
                if rd != ZR {
                    self.clobber();
                    self.load_g(T0, rn);
                    self.a.mov_rr(true, T1, T0);
                    self.a.shift_i(Shf::Sar, sf, T1, if sf { 63 } else { 31 });
                    self.a.alu_rr(Alu::Xor, sf, T0, T1);
                    self.a.lzcnt(sf, T0, T0);
                    let d = self.dst(rd, T1);
                    self.a.lea(sf, d, Mem::b(T0, -1));
                    self.commit(rd, d);
                }
            }
            Insn::Rbit { sf, rd, rn } => {
                if matches!(rest.get(1), Some(Insn::Clz { sf: s2, rd: r2, rn: n2 }) if *s2 == sf && *n2 == rd && *r2 == rd && rd != ZR) {
                    self.clobber();
                    let s = self.src(rn, T0);
                    let d = self.dst(rd, T0);
                    self.a.tzcnt(sf, d, s);
                    self.commit(rd, d);
                    return 2;
                }
                self.rbit(sf, rd, rn);
            }
            Insn::Rev { sf, rd, rn, sz } => self.rev(sf, rd, rn, sz),
            Insn::Crc { c: _, sz, rd, rn, rm } => {
                if rd != ZR {
                    self.clobber();
                    let real = self.dst(rd, T0);
                    let d = if rm != ZR && self.pin(rm) == Some(real) && rn != rm { T0 } else { real };
                    self.load_g(d, rn);
                    let s = self.src(rm, T1);
                    self.a.crc32(sz, d, s);
                    if d != real {
                        self.a.mov_rr(true, real, d);
                    }
                    self.commit(rd, real);
                }
            }
            Insn::Ld { size, sext, rt, rn, addr } => self.ld(size, sext, rt, rn, addr),
            Insn::St { size, rt, rn, addr } => self.st(size, rt, rn, addr),
            Insn::Ldp { sz, sx, rt, rt2, rn, off, mode } => self.ldp(sz, sx, rt, rt2, rn, off, mode),
            Insn::Stp { sz, rt, rt2, rn, off, mode } => self.stp(sz, rt, rt2, rn, off, mode),
            Insn::Ldex { size, rt, rn, .. } => self.ldex(size, rt, rn),
            Insn::Stex { size, rs, rt, rn, .. } => self.stex(size, rs, rt, rn),
            Insn::Ldar { size, rt, rn } => self.ld(size, 0, rt, rn, Addr::Imm { off: 0, mode: Mode::Offset }),
            Insn::Stlr { size, rt, rn } => self.st(size, rt, rn, Addr::Imm { off: 0, mode: Mode::Offset }),
            Insn::Atomic { op, size, rs, rt, rn } => self.atomic(op, size, rs, rt, rn),
            Insn::Cas { size, rs, rt, rn } => self.cas(size, rs, rt, rn),
            Insn::Prfm | Insn::Casp { .. } => {}
            Insn::B { off } => {
                self.exit_direct(pc.wrapping_add(off as u64), None, self.la_exit(true));
            }
            Insn::Bl { off } => {
                self.mov_imm(30, pc.wrapping_add(4), true);
                self.exit_direct(pc.wrapping_add(off as u64), None, false);
            }
            Insn::Bcond { cond, off } => {
                let t = pc.wrapping_add(off as u64);
                if cond >= 14 {
                    self.exit_direct(t, None, self.la);
                } else {
                    self.need_flags();
                    let cc = self.cc_for(cond);
                    self.exit_direct(t, Some(cc), self.la);
                }
            }
            Insn::Cbz { sf, nz, rt, off } => {
                let t = pc.wrapping_add(off as u64);
                self.clobber();
                match self.rm_of_or_zero(rt) {
                    Some(Rm::R(r)) => self.a.test_rr(sf, r, r),
                    Some(Rm::M(m)) => {
                        if sf {
                            self.a.alu_mi(Alu::Cmp, true, m, 0)
                        } else {
                            self.a.alu_mi(Alu::Cmp, false, m, 0)
                        }
                    }
                    None => {
                        self.a.alu_rr(Alu::Xor, false, T0, T0);
                    }
                }
                self.exit_direct(t, Some(if nz { CC_NE } else { CC_E }), self.la);
            }
            Insn::Tbz { nz, rt, bit, off } => {
                let t = pc.wrapping_add(off as u64);
                self.clobber();
                let r = self.src(rt, T0);
                let (w, top) = if bit == 63 { (true, true) } else if bit == 31 { (false, true) } else { (bit >= 32, false) };
                if top {
                    self.a.test_rr(w, r, r);
                    self.exit_direct(t, Some(if nz { CC_S } else { CC_NS }), self.la);
                } else {
                    self.a.bt_ri(w, r, bit);
                    self.exit_direct(t, Some(if nz { CC_B } else { CC_AE }), self.la);
                }
            }
            Insn::Br { rn } | Insn::Ret { rn } => {
                if self.la {
                    self.spill();
                }
                let r = self.src(rn, T0);
                if r != RAX {
                    self.a.mov_rr(true, RAX, r);
                }
                let at = self.a.jmp32() as u32;
                self.fixes.push(Fix::Shared { cold: false, at, id: Shared::Indirect });
            }
            Insn::Blr { rn } => {
                if self.la {
                    self.spill();
                }
                self.load_g(T1, rn);
                self.mov_imm(30, pc.wrapping_add(4), true);
                self.a.mov_rr(true, RAX, T1);
                let at = self.a.jmp32() as u32;
                self.fixes.push(Fix::Shared { cold: false, at, id: Shared::Indirect });
            }
            Insn::Hint { .. } => {}
            Insn::Pac { zero } => {
                if let Some(rd) = zero {
                    self.mov_imm(rd, 0, true);
                }
            }
            Insn::Barrier { kind, crm } => match kind {
                BarKind::Dsb => self.a.mfence(),
                BarKind::Dmb => {
                    if crm & 3 == 3 {
                        self.a.mfence()
                    }
                }
                _ => {}
            },
            Insn::Mrs { rt, id } => {
                if let Some(s) = sys_slot(id) {
                    if rt != ZR {
                        let d = self.dst(rt, T0);
                        self.a.op(0, true, &[0x8b], d, Rm::M(ctxm(OFF_SYS + 8 * s as i32)), false);
                        self.commit(rt, d);
                    }
                }
            }
            Insn::Msr { rt, id } => {
                if let Some(s) = sys_slot(id) {
                    let m = ctxm(OFF_SYS + 8 * s as i32);
                    if rt == ZR {
                        self.a.store_i(3, m, 0);
                    } else {
                        let r = self.src(rt, T0);
                        self.a.store(3, m, r);
                    }
                }
            }
            _ => self.vinsn(&i),
        }
        1
    }

    fn la_exit(&self, _x: bool) -> bool {
        self.la
    }

    fn rm_of_or_zero(&mut self, g: u8) -> Option<Rm> {
        if g == ZR {
            None
        } else {
            Some(self.rm_of(g))
        }
    }

    fn mov_imm(&mut self, rd: u8, v: u64, sf: bool) {
        if rd == ZR {
            return;
        }
        let v = if sf { v } else { v & 0xffff_ffff };
        match self.pin(rd) {
            Some(h) => self.a.mov_ri(h, v),
            None => {
                if fits32(v as i64) {
                    self.a.store_i(3, gslot(rd), v as i64 as i32);
                } else if v <= u32::MAX as u64 {
                    self.a.mov_ri(T0, v);
                    self.a.store(3, gslot(rd), T0);
                } else {
                    self.a.mov_ri(T0, v);
                    self.a.store(3, gslot(rd), T0);
                }
            }
        }
    }

    fn movk(&mut self, sf: bool, rd: u8, imm16: u16, hw: u8) {
        if rd == ZR {
            return;
        }
        self.clobber();
        let sh = 16 * hw as u32;
        let d = self.dst(rd, T0);
        self.load_g(d, rd);
        if hw == 0 && sf {
            self.a.b(0x66);
            let rex = 0x40 | ((d >> 3) & 1);
            if rex != 0x40 {
                self.a.b(rex);
            }
            self.a.b(0xb8 | (d & 7));
            self.a.buf().extend_from_slice(&imm16.to_le_bytes());
        } else {
            let mask = !(0xffffu64 << sh);
            let ins = (imm16 as u64) << sh;
            self.a.mov_ri(T1, if sf { mask } else { mask & 0xffff_ffff });
            self.a.alu_rr(Alu::And, sf, d, T1);
            self.a.mov_ri(T1, ins);
            self.a.alu_rr(Alu::Or, sf, d, T1);
        }
        self.commit(rd, d);
    }

    fn bb(&mut self, b: B, w: bool) -> Bb {
        match b {
            B::G(ZR) => Bb::Imm(0),
            B::G(g) => match self.pin(g) {
                Some(h) => Bb::Reg(h),
                None => Bb::Slot(gslot(g)),
            },
            B::I(v) => {
                let v = if w { v } else { v as u32 as i32 as i64 };
                if fits32(v) {
                    Bb::Imm(v as i32)
                } else {
                    self.a.mov_ri(T1, v as u64);
                    Bb::Reg(T1)
                }
            }
            B::H(h) => Bb::Reg(h),
        }
    }

    fn alu_b(&mut self, op: Alu, w: bool, d: u8, b: Bb) {
        match b {
            Bb::Reg(r) => self.a.alu_rr(op, w, d, r),
            Bb::Slot(m) => self.a.alu_rm(op, w, d, m),
            Bb::Imm(i) => self.a.alu_ri(op, w, d, i),
        }
    }

    fn alu3(&mut self, op: Alu, w: bool, rd: u8, rn: u8, b: B, setf: bool) {
        let defs = setf && self.la;
        if setf && !self.la && rd == ZR {
            return;
        }
        if rd == ZR && !setf {
            return;
        }
        let rd_zero = rd == ZR;
        if !defs && (op == Alu::Add || op == Alu::Sub) {
            if let B::I(v) = b {
                let v = if op == Alu::Sub { -v } else { v };
                let v = if w { v } else { v as u32 as i32 as i64 };
                if rn != ZR && fits32(v) {
                    let d = self.dst(rd, T0);
                    let s = self.src(rn, T0);
                    if v == 0 {
                        if d != s || !w {
                            self.a.mov_rr(w, d, s);
                        }
                    } else {
                        self.a.lea(w, d, Mem::b(s, v as i32));
                    }
                    self.commit(rd, d);
                    return;
                }
            }
            if op == Alu::Add {
                if let B::G(g) = b {
                    if rn != ZR && g != ZR {
                        let a1 = self.src(rn, T1);
                        let a2 = self.src(g, T2);
                        let d = self.dst(rd, T0);
                        self.a.lea(w, d, Mem::bi(a1, a2, 0, 0));
                        self.commit(rd, d);
                        return;
                    }
                }
            }
        }
        if !defs {
            self.clobber();
        }
        let bv = self.bb(b, w);
        if rd_zero {
            self.cmp_like(op, w, rn, bv);
            return;
        }
        let dh = self.dst(rd, T0);
        if self.pin(rd).is_none() && rd == rn && w {
            match bv {
                Bb::Reg(r) => {
                    self.a.alu_mr(op, true, gslot(rd), r);
                    self.finish_flags(op, defs);
                    return;
                }
                Bb::Imm(i) => {
                    self.a.alu_mi(op, true, gslot(rd), i);
                    self.finish_flags(op, defs);
                    return;
                }
                _ => {}
            }
        }
        let commut = matches!(op, Alu::Add | Alu::And | Alu::Or | Alu::Xor);
        if matches!(bv, Bb::Reg(h) if h == dh) {
            if commut && rn != ZR {
                let rnb = match self.rm_of(rn) {
                    Rm::R(r) => Bb::Reg(r),
                    Rm::M(m) => Bb::Slot(m),
                };
                self.alu_b(op, w, dh, rnb);
            } else {
                let acc = if dh == T0 { T1 } else { T0 };
                self.load_g(acc, rn);
                self.alu_b(op, w, acc, bv);
                self.a.mov_rr(w, dh, acc);
            }
        } else {
            if self.pin(rn) != Some(dh) || rn == ZR {
                self.load_g(dh, rn);
            }
            self.alu_b(op, w, dh, bv);
        }
        self.commit(rd, dh);
        self.finish_flags(op, defs);
    }

    fn cmp_like(&mut self, op: Alu, w: bool, rn: u8, bv: Bb) {
        match op {
            Alu::Add => {
                self.load_g(T0, rn);
                self.alu_b(Alu::Add, w, T0, bv);
                self.set_flags(Fl::Add);
            }
            _ => {
                let is_cmp = op != Alu::And;
                let lhs = if rn == ZR { None } else { Some(self.rm_of(rn)) };
                let both_mem = matches!((lhs, bv), (Some(Rm::M(_)), Bb::Slot(_)));
                match (lhs, both_mem) {
                    (Some(Rm::M(m)), false) => {
                        match (is_cmp, bv) {
                            (true, Bb::Reg(r)) => self.a.alu_mr(Alu::Cmp, w, m, r),
                            (true, Bb::Imm(i)) => self.a.alu_mi(Alu::Cmp, w, m, i),
                            (false, Bb::Reg(r)) => self.a.op(0, w, &[0x85], r, Rm::M(m), false),
                            (false, Bb::Imm(i)) => {
                                self.a.op(0, w, &[0xf7], 0, Rm::M(m), false);
                                self.a.d32(i);
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        let l = match lhs {
                            Some(Rm::R(r)) => r,
                            _ => {
                                self.load_g(T0, rn);
                                T0
                            }
                        };
                        if is_cmp {
                            self.alu_b(Alu::Cmp, w, l, bv);
                        } else {
                            match bv {
                                Bb::Reg(r) => self.a.test_rr(w, l, r),
                                Bb::Imm(i) => self.a.test_ri(w, l, i),
                                Bb::Slot(m) => self.a.op(0, w, &[0x85], l, Rm::M(m), false),
                            }
                        }
                    }
                }
                self.set_flags(if is_cmp { Fl::Sub } else { Fl::Add });
            }
        }
    }

    fn finish_flags(&mut self, op: Alu, defs: bool) {
        if defs {
            self.set_flags(match op {
                Alu::Sub | Alu::Cmp | Alu::Sbb => Fl::Sub,
                _ => Fl::Add,
            });
        }
    }

    fn shifted(&mut self, w: bool, rm: u8, sh: Sh, amt: u8, tmp: u8) -> u8 {
        if amt == 0 || rm == ZR {
            if rm == ZR {
                self.a.mov_ri(tmp, 0);
                return tmp;
            }
            return self.src(rm, tmp);
        }
        self.load_g(tmp, rm);
        let k = match sh {
            Sh::Lsl => Shf::Shl,
            Sh::Lsr => Shf::Shr,
            Sh::Asr => Shf::Sar,
            Sh::Ror => Shf::Ror,
        };
        self.a.shift_i(k, w, tmp, amt);
        tmp
    }

    fn addsub_sh(&mut self, sf: bool, sub: bool, set: bool, rd: u8, rn: u8, rm: u8, sh: Sh, amt: u8) {
        let op = if sub { Alu::Sub } else { Alu::Add };
        let defs = set && self.la;
        if amt == 0 {
            self.alu3(op, sf, rd, rn, B::G(rm), set);
            return;
        }
        if !defs && !sub && sh == Sh::Lsl && amt <= 3 && rd != ZR && rn != ZR && rm != ZR {
            let a1 = self.src(rn, T1);
            let a2 = self.src(rm, T2);
            let d = self.dst(rd, T0);
            self.a.lea(sf, d, Mem::bi(a1, a2, amt, 0));
            self.commit(rd, d);
            return;
        }
        if set && !self.la && rd == ZR {
            return;
        }
        self.clobber_unless(defs);
        let t = self.shifted(sf, rm, sh, amt, T2);
        self.alu3(op, sf, rd, rn, B::H(t), set);
    }

    fn clobber_unless(&mut self, defs: bool) {
        if !defs {
            self.clobber();
        }
    }

    fn extend(&mut self, ext: Ext, sf: bool, rm: u8, tmp: u8) -> u8 {
        let s = self.src(rm, tmp);
        match ext {
            Ext::Uxtb => self.a.movzx_rr(0, tmp, s),
            Ext::Uxth => self.a.movzx_rr(1, tmp, s),
            Ext::Uxtw => self.a.mov_rr(false, tmp, s),
            Ext::Sxtb => self.a.movsx_rr(0, sf, tmp, s),
            Ext::Sxth => self.a.movsx_rr(1, sf, tmp, s),
            Ext::Sxtw => self.a.movsx_rr(2, sf, tmp, s),
            Ext::Uxtx | Ext::Sxtx => return s,
        }
        tmp
    }

    fn addsub_ext(&mut self, sf: bool, sub: bool, set: bool, rd: u8, rn: u8, rm: u8, ext: Ext, amt: u8) {
        let op = if sub { Alu::Sub } else { Alu::Add };
        let defs = set && self.la;
        if set && !self.la && rd == ZR {
            return;
        }
        if rm == ZR {
            self.alu3(op, sf, rd, rn, B::I(0), set);
            return;
        }
        let plain = matches!(ext, Ext::Uxtx | Ext::Sxtx) || (!sf && matches!(ext, Ext::Uxtw | Ext::Sxtw));
        if plain && amt == 0 {
            self.alu3(op, sf, rd, rn, B::G(rm), set);
            return;
        }
        let e = self.extend(ext, sf, rm, T2);
        if !defs && !sub && amt <= 3 && rd != ZR && rn != ZR {
            let a1 = self.src(rn, T1);
            let d = self.dst(rd, T0);
            self.a.lea(sf, d, Mem::bi(a1, e, amt, 0));
            self.commit(rd, d);
            return;
        }
        self.clobber_unless(defs);
        let mut t = e;
        if amt != 0 {
            if t != T2 {
                self.a.mov_rr(true, T2, t);
                t = T2;
            }
            self.a.shift_i(Shf::Shl, sf, t, amt);
        }
        self.alu3(op, sf, rd, rn, B::H(t), set);
    }

    fn addsubc(&mut self, sf: bool, sub: bool, set: bool, rd: u8, rn: u8, rm: u8) {
        let defs = set && self.la;
        if rd == ZR && !defs {
            return;
        }
        self.need_flags();
        if !defs {
            self.spill_keep();
        }
        if sub {
            if self.fl == Fl::Add {
                self.a.cmc();
                self.fl = Fl::Sub;
            }
        } else if self.fl == Fl::Sub {
            self.a.cmc();
            self.fl = Fl::Add;
        }
        let dh = if rd == ZR { T0 } else { self.dst(rd, T0) };
        let bv = self.bb(B::G(rm), sf);
        let b_reg = match bv {
            Bb::Reg(r) => Some(r),
            _ => None,
        };
        let acc = if b_reg == Some(dh) { if dh == T0 { T1 } else { T0 } } else { dh };
        self.load_g(acc, rn);
        self.alu_b(if sub { Alu::Sbb } else { Alu::Adc }, sf, acc, bv);
        if acc != dh {
            self.a.mov_rr(sf, dh, acc);
        }
        if sf || true {
            if !sf {
                self.a.mov_rr(false, dh, dh);
            }
        }
        if rd != ZR {
            self.commit(rd, dh);
        }
        if defs {
            self.set_flags(if sub { Fl::Sub } else { Fl::Add });
        } else {
            self.fl = Fl::Inv;
        }
    }

    fn spill_keep(&mut self) {
        if self.la && self.fl != Fl::Inv && !self.saved {
            let was = self.fl;
            if was == Fl::Add {
                self.a.cmc();
                self.fl = Fl::Sub;
            }
            self.a.pushfq();
            self.a.pop_m(ctxm(OFF_FLAGS));
            self.saved = true;
        }
    }

    fn logic_imm(&mut self, sf: bool, alu: Alu, setf: bool, rd: u8, rn: u8, v: i64, raw: u64) {
        let defs = setf && self.la;
        if setf && !self.la && rd == ZR {
            return;
        }
        if alu == Alu::Or && rn == ZR && !setf {
            self.mov_imm(rd, raw, sf);
            return;
        }
        if alu == Alu::And && !setf && rd != ZR && rn != ZR && raw == 0xffff_ffff {
            let s = self.src(rn, T0);
            let d = self.dst(rd, T0);
            self.a.mov_rr(false, d, s);
            self.commit(rd, d);
            return;
        }
        if alu == Alu::Xor && !setf && v == -1 && rd != ZR && rn != ZR {
            let s = self.src(rn, T0);
            let d = self.dst(rd, T0);
            if d != s {
                self.a.mov_rr(true, d, s);
            }
            self.a.not(sf, d);
            self.commit(rd, d);
            return;
        }
        let _ = defs;
        self.alu3(alu, sf, rd, rn, B::I(v), setf);
    }

    fn logsh(&mut self, sf: bool, op: LogOp, inv: bool, rd: u8, rn: u8, rm: u8, sh: Sh, amt: u8) {
        let setf = op == LogOp::Ands;
        let defs = setf && self.la;
        if setf && !self.la && rd == ZR {
            return;
        }
        let alu = match op {
            LogOp::And | LogOp::Ands => Alu::And,
            LogOp::Orr => Alu::Or,
            LogOp::Eor => Alu::Xor,
        };
        if !inv {
            if amt == 0 {
                if op == LogOp::Orr && rn == ZR && rd != ZR {
                    let d = self.dst(rd, T0);
                    self.load_g(d, rm);
                    if !sf {
                        self.a.mov_rr(false, d, d);
                    }
                    self.commit(rd, d);
                    return;
                }
                self.alu3(alu, sf, rd, rn, B::G(rm), setf);
                return;
            }
            self.clobber_unless(defs);
            let t = self.shifted(sf, rm, sh, amt, T2);
            self.alu3(alu, sf, rd, rn, B::H(t), setf);
            return;
        }
        if op == LogOp::And || op == LogOp::Ands {
            if rm != ZR && rn != ZR && rd != ZR || (setf && rd == ZR && rn != ZR && rm != ZR) {
                self.clobber_unless(defs);
                let t = if amt != 0 { self.shifted(sf, rm, sh, amt, T2) } else { self.src(rm, T2) };
                let n = self.rm_of(rn);
                let d = if rd == ZR { T0 } else { self.dst(rd, T0) };
                self.a.andn(sf, d, t, n);
                if rd != ZR {
                    self.commit(rd, d);
                }
                if defs {
                    self.set_flags(Fl::Add);
                }
                return;
            }
        }
        self.clobber_unless(defs);
        let t = if amt != 0 {
            self.shifted(sf, rm, sh, amt, T2)
        } else if rm == ZR {
            self.a.mov_ri(T2, 0);
            T2
        } else {
            let s = self.src(rm, T2);
            if s != T2 {
                self.a.mov_rr(true, T2, s);
            }
            T2
        };
        self.a.not(sf, t);
        if rn == ZR && alu == Alu::Or && !setf && rd != ZR {
            let d = self.dst(rd, T0);
            self.a.mov_rr(sf, d, t);
            self.commit(rd, d);
            return;
        }
        self.alu3(alu, sf, rd, rn, B::H(t), setf);
    }

    fn bfm(&mut self, sf: bool, op: u8, rd: u8, rn: u8, immr: u32, imms: u32) {
        if rd == ZR {
            return;
        }
        let size = if sf { 64 } else { 32 };
        match op {
            2 => {
                if imms != size - 1 && imms + 1 == immr {
                    let n = size - immr;
                    self.shift_by(sf, rd, rn, Shf::Shl, n as u8);
                } else if imms == size - 1 {
                    self.shift_by(sf, rd, rn, Shf::Shr, immr as u8);
                } else if immr == 0 && (imms == 7 || imms == 15) {
                    let s = self.src(rn, T0);
                    let d = self.dst(rd, T0);
                    self.a.movzx_rr(if imms == 7 { 0 } else { 1 }, d, s);
                    self.commit(rd, d);
                } else if imms >= immr {
                    let width = imms - immr + 1;
                    self.clobber();
                    let d = self.dst(rd, T0);
                    self.load_g(d, rn);
                    if immr != 0 {
                        self.a.shift_i(Shf::Shr, sf, d, immr as u8);
                    }
                    self.and_low(sf, d, width);
                    self.commit(rd, d);
                } else {
                    let width = imms + 1;
                    let l = size - immr;
                    self.clobber();
                    let d = self.dst(rd, T0);
                    self.load_g(d, rn);
                    self.and_low(sf, d, width);
                    self.a.shift_i(Shf::Shl, sf, d, l as u8);
                    self.commit(rd, d);
                }
            }
            0 => {
                if imms == size - 1 {
                    self.shift_by(sf, rd, rn, Shf::Sar, immr as u8);
                } else if immr == 0 && (imms == 7 || imms == 15 || imms == 31) {
                    let s = self.src(rn, T0);
                    let d = self.dst(rd, T0);
                    let sz = if imms == 7 { 0 } else if imms == 15 { 1 } else { 2 };
                    self.a.movsx_rr(sz, sf, d, s);
                    self.commit(rd, d);
                } else if imms >= immr {
                    let width = imms - immr + 1;
                    self.clobber();
                    let d = self.dst(rd, T0);
                    self.load_g(d, rn);
                    self.a.shift_i(Shf::Shl, sf, d, (size - 1 - imms) as u8);
                    self.a.shift_i(Shf::Sar, sf, d, (size - width) as u8);
                    self.commit(rd, d);
                } else {
                    let width = imms + 1;
                    let l = size - immr;
                    self.clobber();
                    let d = self.dst(rd, T0);
                    self.load_g(d, rn);
                    self.a.shift_i(Shf::Shl, sf, d, (size - width) as u8);
                    self.a.shift_i(Shf::Sar, sf, d, (size - width - l) as u8);
                    if !sf {
                        self.a.mov_rr(false, d, d);
                    }
                    self.commit(rd, d);
                }
            }
            _ => {
                self.clobber();
                let (width, lsb, shr) = if imms >= immr {
                    (imms - immr + 1, 0u32, immr)
                } else {
                    (imms + 1, size - immr, 0)
                };
                self.load_g(T1, rn);
                if shr != 0 {
                    self.a.shift_i(Shf::Shr, sf, T1, shr as u8);
                }
                self.and_low(sf, T1, width);
                if lsb != 0 {
                    self.a.shift_i(Shf::Shl, sf, T1, lsb as u8);
                }
                let wmask: u64 = if width >= 64 { !0 } else { ((1u64 << width) - 1) << lsb };
                let wmask = if sf { wmask } else { wmask & 0xffff_ffff };
                let keep = !wmask & if sf { !0 } else { 0xffff_ffffu64 };
                let d = self.dst(rd, T0);
                self.load_g(d, rd);
                let keepi = if sf { keep as i64 } else { keep as u32 as i32 as i64 };
                if fits32(keepi) {
                    self.a.alu_ri(Alu::And, sf, d, keepi as i32);
                } else {
                    self.a.mov_ri(T2, keep);
                    self.a.alu_rr(Alu::And, sf, d, T2);
                }
                self.a.alu_rr(Alu::Or, sf, d, T1);
                if !sf {
                    self.a.mov_rr(false, d, d);
                }
                self.commit(rd, d);
            }
        }
    }

    fn and_low(&mut self, sf: bool, d: u8, width: u32) {
        let size = if sf { 64 } else { 32 };
        if width >= size {
            if !sf {
                self.a.mov_rr(false, d, d);
            }
            return;
        }
        if width == 8 {
            self.a.movzx_rr(0, d, d);
        } else if width == 16 {
            self.a.movzx_rr(1, d, d);
        } else if width == 32 {
            self.a.mov_rr(false, d, d);
        } else if width < 32 {
            self.a.alu_ri(Alu::And, false, d, ((1u32 << width) - 1) as i32);
        } else {
            self.a.mov_ri(T2, (1u64 << width) - 1);
            self.a.alu_rr(Alu::And, true, d, T2);
        }
    }

    fn shift_by(&mut self, sf: bool, rd: u8, rn: u8, k: Shf, n: u8) {
        if rd == ZR {
            return;
        }
        if n == 0 && sf {
            let d = self.dst(rd, T0);
            self.load_g(d, rn);
            self.commit(rd, d);
            return;
        }
        if rn == ZR {
            self.mov_imm(rd, 0, true);
            return;
        }
        if k == Shf::Shl && n <= 3 && !self.la_blocks_clobber() && self.pin(rn).is_some() && n != 0 {
            let s = self.pin(rn).unwrap();
            let d = self.dst(rd, T0);
            self.a.lea(sf, d, Mem { base: NOIDX, index: s, scale: n, disp: 0 });
            self.commit(rd, d);
            return;
        }
        self.clobber();
        let d = self.dst(rd, T0);
        self.load_g(d, rn);
        self.a.shift_i(k, sf, d, n);
        self.commit(rd, d);
    }

    fn la_blocks_clobber(&self) -> bool {
        false
    }

    fn extr(&mut self, sf: bool, rd: u8, rn: u8, rm: u8, lsb: u8) {
        if rd == ZR {
            return;
        }
        if rn == rm && rn != ZR {
            let d = self.dst(rd, T0);
            let s = self.rm_of(rn);
            self.a.rorx(sf, d, s, lsb);
            self.commit(rd, d);
            return;
        }
        self.clobber();
        self.load_g(T0, rm);
        let n = self.src(rn, T1);
        if lsb == 0 {
            if !sf {
                self.a.mov_rr(false, T0, T0);
            }
        } else {
            self.a.shrd_i(sf, T0, n, lsb);
        }
        let d = self.dst(rd, T0);
        if d != T0 {
            self.a.mov_rr(sf, d, T0);
        }
        self.commit(rd, d);
    }

    fn shv(&mut self, sf: bool, sh: Sh, rd: u8, rn: u8, rm: u8) {
        if rd == ZR {
            return;
        }
        let d = self.dst(rd, T0);
        if sh != Sh::Ror {
            if rn == ZR {
                self.mov_imm(rd, 0, true);
                return;
            }
            let cnt = self.src(rm, T1);
            let s = self.rm_of(rn);
            match sh {
                Sh::Lsl => self.a.shlx(sf, d, s, cnt),
                Sh::Lsr => self.a.shrx(sf, d, s, cnt),
                _ => self.a.sarx(sf, d, s, cnt),
            }
            self.commit(rd, d);
            return;
        }
        self.clobber();
        self.load_g(T2, rm);
        self.a.neg(sf, T2);
        self.load_g(T1, rn);
        self.a.shlx(sf, T2, Rm::R(T1), T2);
        let cnt = self.src(rm, T0);
        self.a.shrx(sf, T0, Rm::R(T1), cnt);
        self.a.alu_rr(Alu::Or, sf, T0, T2);
        let d = self.dst(rd, T0);
        if d != T0 {
            self.a.mov_rr(sf, d, T0);
        }
        self.commit(rd, d);
    }

    fn madd(&mut self, sf: bool, sub: bool, rd: u8, rn: u8, rm: u8, ra: u8) {
        if rd == ZR {
            return;
        }
        self.clobber();
        if rn == ZR || rm == ZR {
            if ra == ZR || sub && false {
                self.mov_imm(rd, 0, true);
            } else if sub {
                self.alu3(Alu::Sub, sf, rd, ra, B::I(0), false);
            } else {
                let d = self.dst(rd, T0);
                self.load_g(d, ra);
                if !sf {
                    self.a.mov_rr(false, d, d);
                }
                self.commit(rd, d);
            }
            return;
        }
        self.load_g(T0, rn);
        match self.rm_of(rm) {
            Rm::R(r) => self.a.imul_rr(sf, T0, r),
            Rm::M(m) => self.a.imul_rm(sf, T0, m),
        }
        if ra != ZR {
            if sub {
                self.load_g(T1, ra);
                self.a.alu_rr(Alu::Sub, sf, T1, T0);
                self.a.mov_rr(sf, T0, T1);
            } else {
                let b = self.bb(B::G(ra), sf);
                self.alu_b(Alu::Add, sf, T0, b);
            }
        } else if sub {
            self.a.neg(sf, T0);
        }
        let d = self.dst(rd, T0);
        if d != T0 {
            self.a.mov_rr(sf, d, T0);
        }
        self.commit(rd, d);
    }

    fn mull(&mut self, signed: bool, sub: bool, rd: u8, rn: u8, rm: u8, ra: u8) {
        if rd == ZR {
            return;
        }
        self.clobber();
        let sx = |s: &mut Tx, t: u8, g: u8| {
            if g == ZR {
                s.a.mov_ri(t, 0);
            } else {
                let r = s.src(g, t);
                if signed {
                    s.a.movsx_rr(2, true, t, r);
                } else {
                    s.a.mov_rr(false, t, r);
                }
            }
        };
        sx(self, T0, rn);
        sx(self, T1, rm);
        self.a.imul_rr(true, T0, T1);
        if ra != ZR {
            if sub {
                self.load_g(T1, ra);
                self.a.alu_rr(Alu::Sub, true, T1, T0);
                self.a.mov_rr(true, T0, T1);
            } else {
                let b = self.bb(B::G(ra), true);
                self.alu_b(Alu::Add, true, T0, b);
            }
        } else if sub {
            self.a.neg(true, T0);
        }
        let d = self.dst(rd, T0);
        if d != T0 {
            self.a.mov_rr(true, d, T0);
        }
        self.commit(rd, d);
    }

    fn mulh(&mut self, signed: bool, rd: u8, rn: u8, rm: u8) {
        if rd == ZR {
            return;
        }
        if rn == ZR || rm == ZR {
            self.mov_imm(rd, 0, true);
            return;
        }
        if !signed {
            self.load_g(RDX, rn);
            let s = self.rm_of(rm);
            let d = self.dst(rd, T2);
            self.a.mulx(true, d, RAX, s);
            self.commit(rd, d);
            return;
        }
        self.clobber();
        self.load_g(RAX, rn);
        let s = self.src(rm, T2);
        self.a.grp3(true, 5, Rm::R(s));
        let d = self.dst(rd, T1);
        if d != RDX {
            self.a.mov_rr(true, d, RDX);
        }
        self.commit(rd, d);
    }

    fn div(&mut self, sf: bool, signed: bool, rd: u8, rn: u8, rm: u8) {
        if rd == ZR {
            return;
        }
        self.clobber();
        self.load_g(T2, rm);
        self.load_g(RAX, rn);
        self.a.test_rr(sf, T2, T2);
        let j1 = self.a.jcc8(CC_NE);
        self.a.alu_rr(Alu::Xor, false, RAX, RAX);
        let jend1 = self.a.jmp8();
        self.a.patch8(j1);
        let mut jend2 = None;
        if signed {
            self.a.alu_ri(Alu::Cmp, sf, T2, -1);
            let j2 = self.a.jcc8(CC_NE);
            self.a.neg(sf, RAX);
            jend2 = Some(self.a.jmp8());
            self.a.patch8(j2);
            self.a.cqo(sf);
            self.a.grp3(sf, 7, Rm::R(T2));
        } else {
            self.a.alu_rr(Alu::Xor, false, RDX, RDX);
            self.a.grp3(sf, 6, Rm::R(T2));
        }
        self.a.patch8(jend1);
        if let Some(j) = jend2 {
            self.a.patch8(j);
        }
        let d = self.dst(rd, RAX);
        if d != RAX {
            self.a.mov_rr(sf, d, RAX);
        }
        self.commit(rd, d);
    }

    fn csel(&mut self, sf: bool, op: u8, rd: u8, rn: u8, rm: u8, cond: u8) {
        if rd == ZR {
            return;
        }
        if cond >= 14 {
            let d = self.dst(rd, T0);
            self.load_g(d, rn);
            if !sf {
                self.a.mov_rr(false, d, d);
            }
            self.commit(rd, d);
            return;
        }
        self.need_flags();
        if op == 1 && rn == ZR && rm == ZR {
            let cc = self.cc_for(cond) ^ 1;
            let d = self.dst(rd, T0);
            self.a.setcc(cc, d);
            self.a.movzx_rr(0, d, d);
            self.commit(rd, d);
            return;
        }
        let cc = self.cc_for(cond);
        let (src_false, mut false_in_t0) = (rm, false);
        match op {
            0 => {}
            1 => {
                if rm == ZR {
                    self.a.mov_ri(T0, 1);
                } else {
                    let r = self.src(rm, T0);
                    self.a.lea(sf, T0, Mem::b(r, 1));
                }
                false_in_t0 = true;
            }
            2 => {
                self.load_g(T0, rm);
                self.a.not(sf, T0);
                false_in_t0 = true;
            }
            _ => {
                self.load_g(T0, rm);
                self.a.not(sf, T0);
                self.a.lea(sf, T0, Mem::b(T0, 1));
                false_in_t0 = true;
            }
        }
        let d = self.dst(rd, T1);
        let n_rm = if rn == ZR { None } else { Some(self.rm_of(rn)) };
        if rn == ZR {
            self.a.mov_ri(T2, 0);
        }
        let n_reg = match n_rm {
            None => Some(T2),
            Some(Rm::R(r)) => Some(r),
            _ => None,
        };
        if false_in_t0 {
            if d == T0 {
                match n_rm {
                    Some(Rm::M(m)) => self.a.cmovcc_m(cc, sf, T0, m),
                    _ => self.a.cmovcc(cc, sf, T0, n_reg.unwrap()),
                }
                self.commit(rd, T0);
                return;
            }
            let mut dd = d;
            if Some(d) == n_reg && rn != ZR {
                dd = T1;
            }
            self.a.mov_rr(true, dd, T0);
            match n_rm {
                Some(Rm::M(m)) => self.a.cmovcc_m(cc, sf, dd, m),
                _ => self.a.cmovcc(cc, sf, dd, n_reg.unwrap()),
            }
            if dd != d {
                self.a.mov_rr(true, d, dd);
            }
            self.commit(rd, d);
            return;
        }
        let _ = src_false;
        if Some(d) == n_reg && rn != ZR {
            match self.rm_of_or_zero(rm) {
                Some(Rm::R(r)) => self.a.cmovcc(cc ^ 1, sf, d, r),
                Some(Rm::M(m)) => self.a.cmovcc_m(cc ^ 1, sf, d, m),
                None => {
                    self.a.mov_ri(T0, 0);
                    self.a.cmovcc(cc ^ 1, sf, d, T0);
                }
            }
            self.commit(rd, d);
            return;
        }
        let m_rm = self.rm_of_or_zero(rm);
        if m_rm == Some(Rm::R(d)) {
        } else {
            match m_rm {
                Some(_) => self.load_g(d, rm),
                None => self.a.mov_ri(d, 0),
            }
        }
        match n_rm {
            Some(Rm::M(m)) => self.a.cmovcc_m(cc, sf, d, m),
            _ => self.a.cmovcc(cc, sf, d, n_reg.unwrap()),
        }
        if !sf {
        }
        self.commit(rd, d);
    }

    fn ccmp(&mut self, sf: bool, neg: bool, imm: bool, rn: u8, rm: u8, nzcv: u8, cond: u8) {
        if !self.la {
            return;
        }
        if cond < 14 {
            self.need_flags();
        }
        let jelse = if cond < 14 {
            let cc = self.cc_for(cond);
            Some(self.a.jcc8(cc ^ 1))
        } else {
            None
        };
        if rn == ZR {
            self.a.mov_ri(T0, 0);
        } else {
            self.load_g(T0, rn);
        }
        let bv = if imm { Bb::Imm(rm as i32) } else { self.bb(B::G(rm), sf) };
        if neg {
            self.alu_b(Alu::Add, sf, T0, bv);
            self.a.cmc();
        } else {
            self.alu_b(Alu::Cmp, sf, T0, bv);
        }
        if let Some(j) = jelse {
            let jend = self.a.jmp8();
            self.a.patch8(j);
            let mut img = self.cfg.eflags_base as i32 & !0x8c1;
            if nzcv & 8 != 0 {
                img |= 0x80;
            }
            if nzcv & 4 != 0 {
                img |= 0x40;
            }
            if nzcv & 2 == 0 {
                img |= 0x1;
            }
            if nzcv & 1 != 0 {
                img |= 0x800;
            }
            self.a.push_i32(img);
            self.a.popfq();
            self.a.patch8(jend);
        }
        self.set_flags(Fl::Sub);
    }

    fn rbit(&mut self, sf: bool, rd: u8, rn: u8) {
        if rd == ZR {
            return;
        }
        self.clobber();
        self.load_g(T0, rn);
        self.a.bswap(sf, T0);
        let steps: [(u64, u8); 3] = [(0x0f0f0f0f0f0f0f0f, 4), (0x3333333333333333, 2), (0x5555555555555555, 1)];
        for (m, s) in steps {
            self.a.mov_rr(true, T1, T0);
            self.a.shift_i(Shf::Shr, sf, T1, s);
            let mk = if sf { m } else { m & 0xffff_ffff };
            self.a.mov_ri(T2, mk);
            self.a.alu_rr(Alu::And, sf, T1, T2);
            self.a.alu_rr(Alu::And, sf, T0, T2);
            self.a.shift_i(Shf::Shl, sf, T0, s);
            self.a.alu_rr(Alu::Or, sf, T0, T1);
        }
        let d = self.dst(rd, T0);
        if d != T0 {
            self.a.mov_rr(sf, d, T0);
        }
        self.commit(rd, d);
    }

    fn rev(&mut self, sf: bool, rd: u8, rn: u8, sz: u8) {
        if rd == ZR {
            return;
        }
        let d = self.dst(rd, T0);
        self.load_g(d, rn);
        match (sf, sz) {
            (_, 2) => self.a.bswap(sf, d),
            (true, 1) => {
                self.a.bswap(true, d);
                self.a.rorx(true, d, Rm::R(d), 32);
            }
            (false, 0) => {
                self.a.bswap(false, d);
                self.a.rorx(false, d, Rm::R(d), 16);
            }
            _ => {
                self.clobber();
                self.a.mov_ri(T2, 0x00ff_00ff_00ff_00ff);
                self.a.mov_rr(true, T1, d);
                self.a.shift_i(Shf::Shr, true, T1, 8);
                self.a.alu_rr(Alu::And, true, T1, T2);
                self.a.alu_rr(Alu::And, true, d, T2);
                self.a.shift_i(Shf::Shl, true, d, 8);
                self.a.alu_rr(Alu::Or, true, d, T1);
            }
        }
        self.commit(rd, d);
    }
}

mod memops;
mod vec;
