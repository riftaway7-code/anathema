pub const ZR: u8 = 31;
pub const SP: u8 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sh {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ext {
    Uxtb,
    Uxth,
    Uxtw,
    Uxtx,
    Sxtb,
    Sxth,
    Sxtw,
    Sxtx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogOp {
    And,
    Orr,
    Eor,
    Ands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomOp {
    Add,
    Clr,
    Eor,
    Set,
    Smax,
    Smin,
    Umax,
    Umin,
    Swp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Offset,
    Pre,
    Post,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Addr {
    Imm { off: i64, mode: Mode },
    Reg { rm: u8, ext: Ext, amt: u8 },
    Lit { off: i64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarKind {
    Dsb,
    Dmb,
    Isb,
    Clrex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExcKind {
    Svc,
    Hvc,
    Smc,
    Brk,
    Hlt,
    Udf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Insn {
    Adr { rd: u8, imm: i64 },
    Adrp { rd: u8, imm: i64 },
    AddSubImm { sf: bool, sub: bool, set: bool, rd: u8, rn: u8, imm: u32 },
    AddSubSh { sf: bool, sub: bool, set: bool, rd: u8, rn: u8, rm: u8, sh: Sh, amt: u8 },
    AddSubExt { sf: bool, sub: bool, set: bool, rd: u8, rn: u8, rm: u8, ext: Ext, amt: u8 },
    AddSubC { sf: bool, sub: bool, set: bool, rd: u8, rn: u8, rm: u8 },
    LogImm { sf: bool, op: LogOp, rd: u8, rn: u8, imm: u64 },
    LogSh { sf: bool, op: LogOp, inv: bool, rd: u8, rn: u8, rm: u8, sh: Sh, amt: u8 },
    MovW { sf: bool, kind: u8, rd: u8, imm16: u16, hw: u8 },
    Bfm { sf: bool, op: u8, rd: u8, rn: u8, immr: u8, imms: u8 },
    Extr { sf: bool, rd: u8, rn: u8, rm: u8, lsb: u8 },
    ShV { sf: bool, sh: Sh, rd: u8, rn: u8, rm: u8 },
    Madd { sf: bool, sub: bool, rd: u8, rn: u8, rm: u8, ra: u8 },
    Mull { signed: bool, sub: bool, rd: u8, rn: u8, rm: u8, ra: u8 },
    Mulh { signed: bool, rd: u8, rn: u8, rm: u8 },
    Div { sf: bool, signed: bool, rd: u8, rn: u8, rm: u8 },
    Csel { sf: bool, op: u8, rd: u8, rn: u8, rm: u8, cond: u8 },
    Ccmp { sf: bool, neg: bool, imm: bool, rn: u8, rm: u8, nzcv: u8, cond: u8 },
    Clz { sf: bool, rd: u8, rn: u8 },
    Cls { sf: bool, rd: u8, rn: u8 },
    Rbit { sf: bool, rd: u8, rn: u8 },
    Rev { sf: bool, rd: u8, rn: u8, sz: u8 },
    Crc { c: bool, sz: u8, rd: u8, rn: u8, rm: u8 },
    Ld { size: u8, sext: u8, rt: u8, rn: u8, addr: Addr },
    St { size: u8, rt: u8, rn: u8, addr: Addr },
    Ldp { sz: u8, sx: bool, rt: u8, rt2: u8, rn: u8, off: i64, mode: Mode },
    Stp { sz: u8, rt: u8, rt2: u8, rn: u8, off: i64, mode: Mode },
    Ldex { size: u8, rt: u8, rt2: u8, rn: u8, pair: bool },
    Stex { size: u8, rs: u8, rt: u8, rt2: u8, rn: u8, pair: bool },
    Ldar { size: u8, rt: u8, rn: u8 },
    Stlr { size: u8, rt: u8, rn: u8 },
    Atomic { op: AtomOp, size: u8, rs: u8, rt: u8, rn: u8 },
    Cas { size: u8, rs: u8, rt: u8, rn: u8 },
    Casp { size: u8, rs: u8, rt: u8, rn: u8 },
    Prfm,
    B { off: i64 },
    Bl { off: i64 },
    Bcond { cond: u8, off: i64 },
    Cbz { sf: bool, nz: bool, rt: u8, off: i64 },
    Tbz { nz: bool, rt: u8, bit: u8, off: i64 },
    Br { rn: u8 },
    Blr { rn: u8 },
    Ret { rn: u8 },
    Eret,
    Hint { op: u8 },
    Pac { zero: Option<u8> },
    Barrier { kind: BarKind, crm: u8 },
    Mrs { rt: u8, id: u16 },
    Msr { rt: u8, id: u16 },
    MsrImm { field: u8, imm: u8 },
    Sys { l: bool, op1: u8, crn: u8, crm: u8, op2: u8, rt: u8 },
    Exc { kind: ExcKind, imm: u16 },
    VLd { size: u8, rt: u8, rn: u8, addr: Addr },
    VSt { size: u8, rt: u8, rn: u8, addr: Addr },
    VLdp { sz: u8, rt: u8, rt2: u8, rn: u8, off: i64, mode: Mode },
    VStp { sz: u8, rt: u8, rt2: u8, rn: u8, off: i64, mode: Mode },
    Movi { q: bool, rd: u8, imm: u64 },
    FmovI { ftype: u8, rd: u8, imm: u64 },
    FmovR { ftype: u8, rd: u8, rn: u8 },
    FmovToG { sf: bool, ftype: u8, top: bool, rd: u8, rn: u8 },
    FmovFromG { sf: bool, ftype: u8, top: bool, rd: u8, rn: u8 },
    Fp1 { ftype: u8, op: u8, rd: u8, rn: u8 },
    Fp2 { ftype: u8, op: u8, rd: u8, rn: u8, rm: u8 },
    Fp3 { ftype: u8, op: u8, rd: u8, rn: u8, rm: u8, ra: u8 },
    Fcmp { ftype: u8, rn: u8, rm: u8, zero: bool, e: bool },
    Fcsel { ftype: u8, rd: u8, rn: u8, rm: u8, cond: u8 },
    Fcvt { from: u8, to: u8, rd: u8, rn: u8 },
    CvtFI { sf: bool, ftype: u8, rd: u8, rn: u8, signed: bool },
    CvtIF { sf: bool, ftype: u8, rd: u8, rn: u8, signed: bool },
    VLog { op: u8, q: bool, rd: u8, rn: u8, rm: u8 },
    VArith { sub: bool, size: u8, q: bool, rd: u8, rn: u8, rm: u8 },
    VDupG { q: bool, size: u8, rd: u8, rn: u8 },
    VDupE { q: bool, size: u8, idx: u8, rd: u8, rn: u8 },
    Umov { sf: bool, size: u8, idx: u8, rd: u8, rn: u8 },
    InsG { size: u8, idx: u8, rd: u8, rn: u8 },
    Unsup(&'static str),
}

#[inline]
fn bits(w: u32, hi: u32, lo: u32) -> u32 {
    (w >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

#[inline]
fn sext(v: u32, n: u32) -> i64 {
    let s = 64 - n;
    ((v as u64) << s) as i64 >> s
}

fn rsp(r: u32) -> u8 {
    if r == 31 {
        SP
    } else {
        r as u8
    }
}

fn bitmask(n: u32, imms: u32, immr: u32, datasize: u32) -> Option<u64> {
    let x = (n << 6) | ((!imms) & 0x3f);
    if x == 0 {
        return None;
    }
    let len = 31 - x.leading_zeros();
    if len < 1 {
        return None;
    }
    let esize = 1u32 << len;
    if esize > datasize {
        return None;
    }
    let levels = esize - 1;
    let s = imms & levels;
    let r = immr & levels;
    if s == levels {
        return None;
    }
    let welem = (1u64 << (s + 1)) - 1;
    let emask = if esize == 64 { !0u64 } else { (1u64 << esize) - 1 };
    let rot = if r == 0 { welem } else { ((welem >> r) | (welem << (esize - r))) & emask };
    let mut v = rot;
    let mut e = esize;
    while e < datasize {
        v |= v << e;
        e *= 2;
    }
    Some(if datasize == 32 { v & 0xffff_ffff } else { v })
}

pub fn decode(w: u32) -> Insn {
    let op0 = (w >> 25) & 0xf;
    match op0 {
        0b1000 | 0b1001 => dp_imm(w),
        0b1010 | 0b1011 => branch(w),
        _ if op0 & 0b0101 == 0b0100 => ldst(w),
        _ if op0 & 0b0111 == 0b0101 => dp_reg(w),
        _ if op0 & 0b0111 == 0b0111 => simd(w),
        0 => {
            if w >> 16 == 0 {
                Insn::Exc { kind: ExcKind::Udf, imm: w as u16 }
            } else {
                Insn::Unsup("sme")
            }
        }
        _ => Insn::Unsup("sve"),
    }
}

mod simd;
use simd::{simd, vldst};

fn dp_imm(w: u32) -> Insn {
    let sf = w >> 31 != 0;
    let rd = bits(w, 4, 0);
    let rn = bits(w, 9, 5);
    match bits(w, 25, 23) {
        0b000 | 0b001 => {
            let imm = sext((bits(w, 23, 5) << 2) | bits(w, 30, 29), 21);
            if sf {
                Insn::Adrp { rd: rd as u8, imm: imm << 12 }
            } else {
                Insn::Adr { rd: rd as u8, imm }
            }
        }
        0b010 => {
            let sub = bits(w, 30, 30) != 0;
            let set = bits(w, 29, 29) != 0;
            let sh = bits(w, 22, 22);
            let imm = bits(w, 21, 10) << (12 * sh);
            Insn::AddSubImm {
                sf,
                sub,
                set,
                rd: if set { rd as u8 } else { rsp(rd) },
                rn: rsp(rn),
                imm,
            }
        }
        0b011 => Insn::Unsup("addg"),
        0b100 => {
            let opc = bits(w, 30, 29);
            let n = bits(w, 22, 22);
            if !sf && n != 0 {
                return Insn::Unsup("logimm-bad");
            }
            let imm = match bitmask(n, bits(w, 15, 10), bits(w, 21, 16), if sf { 64 } else { 32 }) {
                Some(m) => m,
                None => return Insn::Unsup("logimm-bad"),
            };
            let op = [LogOp::And, LogOp::Orr, LogOp::Eor, LogOp::Ands][opc as usize];
            Insn::LogImm {
                sf,
                op,
                rd: if op == LogOp::Ands { rd as u8 } else { rsp(rd) },
                rn: rn as u8,
                imm,
            }
        }
        0b101 => {
            let opc = bits(w, 30, 29);
            let hw = bits(w, 22, 21);
            if opc == 1 || (!sf && hw >= 2) {
                return Insn::Unsup("movw-bad");
            }
            Insn::MovW { sf, kind: opc as u8, rd: rd as u8, imm16: bits(w, 20, 5) as u16, hw: hw as u8 }
        }
        0b110 => {
            let opc = bits(w, 30, 29);
            if opc == 3 || bits(w, 22, 22) != sf as u32 {
                return Insn::Unsup("bfm-bad");
            }
            let immr = bits(w, 21, 16);
            let imms = bits(w, 15, 10);
            if !sf && (immr >= 32 || imms >= 32) {
                return Insn::Unsup("bfm-bad");
            }
            Insn::Bfm { sf, op: opc as u8, rd: rd as u8, rn: rn as u8, immr: immr as u8, imms: imms as u8 }
        }
        _ => {
            let n = bits(w, 22, 22);
            if bits(w, 30, 29) != 0 || bits(w, 21, 21) != 0 || n != sf as u32 {
                return Insn::Unsup("extr-bad");
            }
            let imms = bits(w, 15, 10);
            if !sf && imms >= 32 {
                return Insn::Unsup("extr-bad");
            }
            Insn::Extr { sf, rd: rd as u8, rn: rn as u8, rm: bits(w, 20, 16) as u8, lsb: imms as u8 }
        }
    }
}

fn branch(w: u32) -> Insn {
    if w & 0x7c000000 == 0x14000000 {
        let off = sext(bits(w, 25, 0), 26) << 2;
        return if w >> 31 != 0 { Insn::Bl { off } } else { Insn::B { off } };
    }
    if w & 0x7e000000 == 0x34000000 {
        return Insn::Cbz {
            sf: w >> 31 != 0,
            nz: bits(w, 24, 24) != 0,
            rt: bits(w, 4, 0) as u8,
            off: sext(bits(w, 23, 5), 19) << 2,
        };
    }
    if w & 0x7e000000 == 0x36000000 {
        return Insn::Tbz {
            nz: bits(w, 24, 24) != 0,
            rt: bits(w, 4, 0) as u8,
            bit: ((bits(w, 31, 31) << 5) | bits(w, 23, 19)) as u8,
            off: sext(bits(w, 18, 5), 14) << 2,
        };
    }
    if w & 0xff000010 == 0x54000000 {
        return Insn::Bcond { cond: bits(w, 3, 0) as u8, off: sext(bits(w, 23, 5), 19) << 2 };
    }
    if w & 0xff000000 == 0xd4000000 {
        let imm = bits(w, 20, 5) as u16;
        let opc = bits(w, 23, 21);
        let ll = bits(w, 1, 0);
        return match (opc, ll) {
            (0, 1) if bits(w, 4, 2) == 0 => Insn::Exc { kind: ExcKind::Svc, imm },
            (0, 2) if bits(w, 4, 2) == 0 => Insn::Exc { kind: ExcKind::Hvc, imm },
            (0, 3) if bits(w, 4, 2) == 0 => Insn::Exc { kind: ExcKind::Smc, imm },
            (1, 0) if bits(w, 4, 2) == 0 => Insn::Exc { kind: ExcKind::Brk, imm },
            (2, 0) if bits(w, 4, 2) == 0 => Insn::Exc { kind: ExcKind::Hlt, imm },
            _ => Insn::Unsup("exc"),
        };
    }
    if w & 0xffc00000 == 0xd5000000 {
        return system(w);
    }
    if w & 0xfe000000 == 0xd6000000 {
        let opc = bits(w, 24, 21);
        let op2 = bits(w, 20, 16);
        let op3 = bits(w, 15, 10);
        let rn = bits(w, 9, 5);
        let op4 = bits(w, 4, 0);
        if op2 != 0x1f {
            return Insn::Unsup("br-bad");
        }
        return match (opc, op3, op4) {
            (0, 0, 0) => Insn::Br { rn: rn as u8 },
            (1, 0, 0) => Insn::Blr { rn: rn as u8 },
            (2, 0, 0) => Insn::Ret { rn: rn as u8 },
            (4, 0, 0) if rn == 31 => Insn::Eret,
            (0, 2 | 3, 31) => Insn::Br { rn: rn as u8 },
            (1, 2 | 3, 31) => Insn::Blr { rn: rn as u8 },
            (8, 2 | 3, _) => Insn::Br { rn: rn as u8 },
            (9, 2 | 3, _) => Insn::Blr { rn: rn as u8 },
            (2, 2 | 3, 31) if rn == 31 => Insn::Ret { rn: 30 },
            (4, 2 | 3, 31) if rn == 31 => Insn::Eret,
            _ => Insn::Unsup("br-bad"),
        };
    }
    Insn::Unsup("branch")
}

fn system(w: u32) -> Insn {
    let l = bits(w, 21, 21) != 0;
    let op0 = bits(w, 20, 19);
    let op1 = bits(w, 18, 16);
    let crn = bits(w, 15, 12);
    let crm = bits(w, 11, 8);
    let op2 = bits(w, 7, 5);
    let rt = bits(w, 4, 0);
    if op0 >= 2 {
        let id = bits(w, 19, 5) as u16;
        return if l { Insn::Mrs { rt: rt as u8, id } } else { Insn::Msr { rt: rt as u8, id } };
    }
    if op0 == 1 {
        return Insn::Sys { l, op1: op1 as u8, crn: crn as u8, crm: crm as u8, op2: op2 as u8, rt: rt as u8 };
    }
    if l {
        return Insn::Unsup("system");
    }
    match crn {
        2 if op1 == 3 && rt == 31 => Insn::Hint { op: ((crm << 3) | op2) as u8 },
        3 if op1 != 3 || rt != 31 => Insn::Unsup("barrier-bad"),
        3 => match op2 {
            2 => Insn::Barrier { kind: BarKind::Clrex, crm: crm as u8 },
            4 => Insn::Barrier { kind: BarKind::Dsb, crm: crm as u8 },
            5 => Insn::Barrier { kind: BarKind::Dmb, crm: crm as u8 },
            6 => Insn::Barrier { kind: BarKind::Isb, crm: crm as u8 },
            _ => Insn::Unsup("barrier"),
        },
        4 => Insn::MsrImm { field: ((op1 << 3) | op2) as u8, imm: crm as u8 },
        _ => Insn::Unsup("system"),
    }
}

fn size_ld(size: u32, opc: u32) -> Option<(bool, u8)> {
    match opc {
        0 => Some((false, 0)),
        1 => Some((true, 0)),
        2 => {
            if size == 3 {
                None
            } else {
                Some((true, 2))
            }
        }
        _ => {
            if size >= 2 {
                None
            } else {
                Some((true, 1))
            }
        }
    }
}

fn ldst(w: u32) -> Insn {
    let size = bits(w, 31, 30);
    let v = bits(w, 26, 26) != 0;
    let rt = bits(w, 4, 0);
    let rn = bits(w, 9, 5);
    if w & 0x3b000000 == 0x18000000 {
        if v {
            return vldst(w);
        }
        let off = sext(bits(w, 23, 5), 19) << 2;
        return match size {
            0 => Insn::Ld { size: 2, sext: 0, rt: rt as u8, rn: 0, addr: Addr::Lit { off } },
            1 => Insn::Ld { size: 3, sext: 0, rt: rt as u8, rn: 0, addr: Addr::Lit { off } },
            2 => Insn::Ld { size: 2, sext: 2, rt: rt as u8, rn: 0, addr: Addr::Lit { off } },
            _ => Insn::Prfm,
        };
    }
    if w & 0x3f000000 == 0x08000000 {
        let o2 = bits(w, 23, 23);
        let l = bits(w, 22, 22) != 0;
        let o1 = bits(w, 21, 21);
        let rs = bits(w, 20, 16);
        let o0 = bits(w, 15, 15);
        let rt2 = bits(w, 14, 10);
        let sz = size as u8;
        let rn8 = rsp(rn);
        let sbo_ok = match (o2, o1) {
            (0, 0) => rt2 == 31 && (!l || rs == 31),
            (0, 1) => size < 2 || !l || rs == 31,
            (1, 0) => rt2 == 31 && rs == 31,
            _ => true,
        };
        if !sbo_ok {
            return Insn::Unsup("excl-bad");
        }
        return match (o2, o1) {
            (0, 0) => {
                if l {
                    Insn::Ldex { size: sz, rt: rt as u8, rt2: 31, rn: rn8, pair: false }
                } else {
                    Insn::Stex { size: sz, rs: rs as u8, rt: rt as u8, rt2: 31, rn: rn8, pair: false }
                }
            }
            (0, 1) => {
                if size >= 2 {
                    let s = (size - 1) as u8 + 1;
                    if l {
                        Insn::Ldex { size: s, rt: rt as u8, rt2: rt2 as u8, rn: rn8, pair: true }
                    } else {
                        Insn::Stex { size: s, rs: rs as u8, rt: rt as u8, rt2: rt2 as u8, rn: rn8, pair: true }
                    }
                } else if size == 0 || size == 1 {
                    if rt2 != 31 {
                        return Insn::Unsup("casp-bad");
                    }
                    let _ = o0;
                    Insn::Casp { size: 2 + size as u8, rs: rs as u8, rt: rt as u8, rn: rn8 }
                } else {
                    Insn::Unsup("excl-bad")
                }
            }
            (1, 0) => {
                if l {
                    Insn::Ldar { size: sz, rt: rt as u8, rn: rn8 }
                } else {
                    Insn::Stlr { size: sz, rt: rt as u8, rn: rn8 }
                }
            }
            _ => {
                if rt2 != 31 {
                    return Insn::Unsup("cas-bad");
                }
                Insn::Cas { size: sz, rs: rs as u8, rt: rt as u8, rn: rn8 }
            }
        };
    }
    if w & 0x3a000000 == 0x28000000 {
        if v {
            return vldst(w);
        }
        let opc = size;
        let ty = bits(w, 24, 23);
        let l = bits(w, 22, 22) != 0;
        let imm7 = bits(w, 21, 15);
        let rt2 = bits(w, 14, 10) as u8;
        if ty == 0 && opc == 1 {
            return Insn::Unsup("ldnp-bad");
        }
        let (sz, sx) = match opc {
            0 => (2u8, false),
            1 => {
                if l {
                    (2u8, true)
                } else {
                    return Insn::Unsup("stgp");
                }
            }
            2 => (3u8, false),
            _ => return Insn::Unsup("ldp-bad"),
        };
        let off = sext(imm7, 7) << sz;
        let mode = match ty {
            0 | 2 => Mode::Offset,
            1 => Mode::Post,
            _ => Mode::Pre,
        };
        let rn8 = rsp(rn);
        return if l {
            Insn::Ldp { sz, sx, rt: rt as u8, rt2, rn: rn8, off, mode }
        } else {
            Insn::Stp { sz, rt: rt as u8, rt2, rn: rn8, off, mode }
        };
    }
    if w & 0x3a000000 == 0x38000000 {
        if v {
            return vldst(w);
        }
        let opc = bits(w, 23, 22);
        let rn8 = rsp(rn);
        let b2524 = bits(w, 25, 24);
        if b2524 == 1 {
            let off = (bits(w, 21, 10) as i64) << size;
            return mkldst(size, opc, rt as u8, rn8, Addr::Imm { off, mode: Mode::Offset });
        }
        if bits(w, 21, 21) == 0 {
            let imm9 = sext(bits(w, 20, 12), 9);
            let mode = match bits(w, 11, 10) {
                0 | 2 => Mode::Offset,
                1 => Mode::Post,
                _ => Mode::Pre,
            };
            if mode != Mode::Offset && size == 3 && opc == 2 {
                return Insn::Unsup("prfm-bad");
            }
            return mkldst(size, opc, rt as u8, rn8, Addr::Imm { off: imm9, mode });
        }
        return match bits(w, 11, 10) {
            0 => atomic(w),
            2 => {
                let option = bits(w, 15, 13);
                let ext = match option {
                    2 => Ext::Uxtw,
                    3 => Ext::Uxtx,
                    6 => Ext::Sxtw,
                    7 => Ext::Sxtx,
                    _ => return Insn::Unsup("ldst-bad"),
                };
                let amt = if bits(w, 12, 12) != 0 { size as u8 } else { 0 };
                mkldst(size, opc, rt as u8, rn8, Addr::Reg { rm: bits(w, 20, 16) as u8, ext, amt })
            }
            _ => {
                if size != 3 || bits(w, 26, 26) != 0 || bits(w, 23, 23) > 1 {
                    return Insn::Unsup("ldr-pac-bad");
                }
                let imm = sext((bits(w, 22, 22) << 9) | bits(w, 20, 12), 10) << 3;
                let mode = if bits(w, 11, 11) != 0 { Mode::Pre } else { Mode::Offset };
                Insn::Ld { size: 3, sext: 0, rt: rt as u8, rn: rn8, addr: Addr::Imm { off: imm, mode } }
            }
        };
    }
    if w & 0x3f200c00 == 0x19000000 {
        // RCpc unscaled (LDAPUR/STLUR family). Unlike the ordinary unscaled form, size=11
        // with opc=1x is unallocated here, not PRFUM.
        if size == 3 && bits(w, 23, 22) >= 2 {
            return Insn::Unsup("rcpc-bad");
        }
        let imm9 = sext(bits(w, 20, 12), 9);
        return mkldst(size, bits(w, 23, 22), rt as u8, rsp(rn), Addr::Imm { off: imm9, mode: Mode::Offset });
    }
    if w & 0xbe000000 == 0x0c000000 {
        return Insn::Unsup("simd-ldst-struct");
    }
    Insn::Unsup("ldst")
}

fn mkldst(size: u32, opc: u32, rt: u8, rn: u8, addr: Addr) -> Insn {
    if opc == 0 {
        return Insn::St { size: size as u8, rt, rn, addr };
    }
    match size_ld(size, opc) {
        Some((_, sext)) => Insn::Ld { size: size as u8, sext, rt, rn, addr },
        None => {
            if size == 3 && opc == 2 {
                Insn::Prfm
            } else {
                Insn::Unsup("ldst-bad")
            }
        }
    }
}

fn atomic(w: u32) -> Insn {
    let size = bits(w, 31, 30) as u8;
    let a = bits(w, 23, 23);
    let r = bits(w, 22, 22);
    let rs = bits(w, 20, 16) as u8;
    let o3 = bits(w, 15, 15);
    let opc = bits(w, 14, 12);
    let rn = rsp(bits(w, 9, 5));
    let rt = bits(w, 4, 0) as u8;
    if o3 == 1 {
        if opc == 0 {
            return Insn::Atomic { op: AtomOp::Swp, size, rs, rt, rn };
        }
        if opc == 4 && a == 1 && r == 0 && rs == 31 {
            return Insn::Ldar { size, rt, rn };
        }
        return Insn::Unsup("atomic-bad");
    }
    let op = match opc {
        0 => AtomOp::Add,
        1 => AtomOp::Clr,
        2 => AtomOp::Eor,
        3 => AtomOp::Set,
        4 => AtomOp::Smax,
        5 => AtomOp::Smin,
        6 => AtomOp::Umax,
        _ => AtomOp::Umin,
    };
    Insn::Atomic { op, size, rs, rt, rn }
}

fn shift_of(v: u32) -> Sh {
    [Sh::Lsl, Sh::Lsr, Sh::Asr, Sh::Ror][v as usize]
}

fn dp_reg(w: u32) -> Insn {
    let sf = w >> 31 != 0;
    let rd = bits(w, 4, 0);
    let rn = bits(w, 9, 5);
    let rm = bits(w, 20, 16);
    let top = bits(w, 28, 21);
    if w & 0x1f000000 == 0x0a000000 {
        let amt = bits(w, 15, 10);
        if !sf && amt >= 32 {
            return Insn::Unsup("logsh-bad");
        }
        let op = [LogOp::And, LogOp::Orr, LogOp::Eor, LogOp::Ands][bits(w, 30, 29) as usize];
        return Insn::LogSh {
            sf,
            op,
            inv: bits(w, 21, 21) != 0,
            rd: rd as u8,
            rn: rn as u8,
            rm: rm as u8,
            sh: shift_of(bits(w, 23, 22)),
            amt: amt as u8,
        };
    }
    if w & 0x1f000000 == 0x0b000000 {
        let sub = bits(w, 30, 30) != 0;
        let set = bits(w, 29, 29) != 0;
        if bits(w, 21, 21) == 0 {
            let amt = bits(w, 15, 10);
            let sh = bits(w, 23, 22);
            if sh == 3 || (!sf && amt >= 32) {
                return Insn::Unsup("addsub-bad");
            }
            return Insn::AddSubSh {
                sf,
                sub,
                set,
                rd: rd as u8,
                rn: rn as u8,
                rm: rm as u8,
                sh: shift_of(sh),
                amt: amt as u8,
            };
        }
        if bits(w, 23, 22) != 0 || bits(w, 12, 10) > 4 {
            return Insn::Unsup("addsub-bad");
        }
        let ext = [
            Ext::Uxtb,
            Ext::Uxth,
            Ext::Uxtw,
            Ext::Uxtx,
            Ext::Sxtb,
            Ext::Sxth,
            Ext::Sxtw,
            Ext::Sxtx,
        ][bits(w, 15, 13) as usize];
        return Insn::AddSubExt {
            sf,
            sub,
            set,
            rd: if set { rd as u8 } else { rsp(rd) },
            rn: rsp(rn),
            rm: rm as u8,
            ext,
            amt: bits(w, 12, 10) as u8,
        };
    }
    if top == 0xd0 {
        if bits(w, 15, 10) != 0 {
            return Insn::Unsup("setf");
        }
        return Insn::AddSubC {
            sf,
            sub: bits(w, 30, 30) != 0,
            set: bits(w, 29, 29) != 0,
            rd: rd as u8,
            rn: rn as u8,
            rm: rm as u8,
        };
    }
    if top == 0xd2 {
        if bits(w, 29, 29) == 0 || bits(w, 10, 10) != 0 || bits(w, 4, 4) != 0 {
            return Insn::Unsup("ccmp-bad");
        }
        return Insn::Ccmp {
            sf,
            neg: bits(w, 30, 30) == 0,
            imm: bits(w, 11, 11) != 0,
            rn: rn as u8,
            rm: rm as u8,
            nzcv: bits(w, 3, 0) as u8,
            cond: bits(w, 15, 12) as u8,
        };
    }
    if top == 0xd4 {
        if bits(w, 29, 29) != 0 || bits(w, 11, 11) != 0 {
            return Insn::Unsup("csel-bad");
        }
        let op = (bits(w, 30, 30) << 1) | bits(w, 10, 10);
        return Insn::Csel {
            sf,
            op: op as u8,
            rd: rd as u8,
            rn: rn as u8,
            rm: rm as u8,
            cond: bits(w, 15, 12) as u8,
        };
    }
    if w & 0x1f000000 == 0x1b000000 {
        let op31 = bits(w, 23, 21);
        let o0 = bits(w, 15, 15) != 0;
        let ra = bits(w, 14, 10) as u8;
        if bits(w, 30, 29) != 0 {
            return Insn::Unsup("mul-bad");
        }
        return match (op31, sf) {
            (0, _) => Insn::Madd { sf, sub: o0, rd: rd as u8, rn: rn as u8, rm: rm as u8, ra },
            (1, true) => Insn::Mull { signed: true, sub: o0, rd: rd as u8, rn: rn as u8, rm: rm as u8, ra },
            (5, true) => Insn::Mull { signed: false, sub: o0, rd: rd as u8, rn: rn as u8, rm: rm as u8, ra },
            (2, true) if !o0 => Insn::Mulh { signed: true, rd: rd as u8, rn: rn as u8, rm: rm as u8 },
            (6, true) if !o0 => Insn::Mulh { signed: false, rd: rd as u8, rn: rn as u8, rm: rm as u8 },
            _ => Insn::Unsup("mul-bad"),
        };
    }
    if top == 0xd6 {
        let opcode = bits(w, 15, 10);
        if bits(w, 30, 30) == 0 {
            if bits(w, 29, 29) != 0 {
                return Insn::Unsup("dp2-bad");
            }
            let (rd, rn, rm) = (rd as u8, rn as u8, rm as u8);
            return match opcode {
                2 => Insn::Div { sf, signed: false, rd, rn, rm },
                3 => Insn::Div { sf, signed: true, rd, rn, rm },
                8 => Insn::ShV { sf, sh: Sh::Lsl, rd, rn, rm },
                9 => Insn::ShV { sf, sh: Sh::Lsr, rd, rn, rm },
                10 => Insn::ShV { sf, sh: Sh::Asr, rd, rn, rm },
                11 => Insn::ShV { sf, sh: Sh::Ror, rd, rn, rm },
                12 if sf => Insn::Pac { zero: Some(rd) },
                16..=23 => {
                    let sz = (opcode & 3) as u8;
                    if (sz == 3) != sf {
                        return Insn::Unsup("crc-bad");
                    }
                    Insn::Crc { c: opcode & 4 != 0, sz, rd, rn, rm }
                }
                _ => Insn::Unsup("dp2"),
            };
        }
        if bits(w, 29, 29) != 0 {
            return Insn::Unsup("dp1-bad");
        }
        let opcode2 = rm;
        let (rd8, rn8) = (rd as u8, rn as u8);
        if opcode2 == 1 {
            // PAC group, 64-bit only. Opcodes 0-7 (PACIA..AUTDB) take a modifier in Rn; the
            // zero-modifier forms (8-15) and XPACI/XPACD (16, 17) need Rn = 31. The rest is
            // unallocated (checked against an Apple M3: all of it raises SIGILL).
            let ok = sf
                && match opcode {
                    0..=7 => true,
                    8..=17 => rn == 31,
                    _ => false,
                };
            return if ok { Insn::Pac { zero: None } } else { Insn::Unsup("pac-bad") };
        }
        if opcode2 != 0 {
            return Insn::Unsup("dp1-bad");
        }
        return match (opcode, sf) {
            (0, _) => Insn::Rbit { sf, rd: rd8, rn: rn8 },
            (1, _) => Insn::Rev { sf, rd: rd8, rn: rn8, sz: 0 },
            (2, false) => Insn::Rev { sf, rd: rd8, rn: rn8, sz: 2 },
            (2, true) => Insn::Rev { sf, rd: rd8, rn: rn8, sz: 1 },
            (3, true) => Insn::Rev { sf, rd: rd8, rn: rn8, sz: 2 },
            (4, _) => Insn::Clz { sf, rd: rd8, rn: rn8 },
            (5, _) => Insn::Cls { sf, rd: rd8, rn: rn8 },
            _ => Insn::Unsup("dp1"),
        };
    }
    Insn::Unsup("dpreg")
}

impl Insn {

    pub fn reg_uses(&self, f: &mut dyn FnMut(u8)) {
        let mut g = |r: u8| {
            if r != ZR {
                f(r)
            }
        };
        match *self {
            Insn::Adr { rd, .. } | Insn::Adrp { rd, .. } | Insn::MovW { rd, .. } => g(rd),
            Insn::AddSubImm { rd, rn, .. } | Insn::LogImm { rd, rn, .. } | Insn::Bfm { rd, rn, .. } => {
                g(rd);
                g(rn)
            }
            Insn::Clz { rd, rn, .. } | Insn::Cls { rd, rn, .. } | Insn::Rbit { rd, rn, .. } | Insn::Rev { rd, rn, .. } => {
                g(rd);
                g(rn)
            }
            Insn::AddSubSh { rd, rn, rm, .. }
            | Insn::AddSubExt { rd, rn, rm, .. }
            | Insn::AddSubC { rd, rn, rm, .. }
            | Insn::LogSh { rd, rn, rm, .. }
            | Insn::Extr { rd, rn, rm, .. }
            | Insn::ShV { rd, rn, rm, .. }
            | Insn::Mulh { rd, rn, rm, .. }
            | Insn::Div { rd, rn, rm, .. }
            | Insn::Csel { rd, rn, rm, .. }
            | Insn::Crc { rd, rn, rm, .. } => {
                g(rd);
                g(rn);
                g(rm)
            }
            Insn::Madd { rd, rn, rm, ra, .. } | Insn::Mull { rd, rn, rm, ra, .. } => {
                g(rd);
                g(rn);
                g(rm);
                g(ra)
            }
            Insn::Ccmp { rn, rm, imm, .. } => {
                g(rn);
                if !imm {
                    g(rm)
                }
            }
            Insn::Ld { rt, rn, addr, .. } | Insn::St { rt, rn, addr, .. } => {
                g(rt);
                if !matches!(addr, Addr::Lit { .. }) {
                    g(rn)
                }
                if let Addr::Reg { rm, .. } = addr {
                    g(rm)
                }
            }
            Insn::Ldp { rt, rt2, rn, .. } | Insn::Stp { rt, rt2, rn, .. } => {
                g(rt);
                g(rt2);
                g(rn)
            }
            Insn::Ldex { rt, rn, .. } | Insn::Ldar { rt, rn, .. } | Insn::Stlr { rt, rn, .. } => {
                g(rt);
                g(rn)
            }
            Insn::Stex { rs, rt, rn, .. } | Insn::Atomic { rs, rt, rn, .. } | Insn::Cas { rs, rt, rn, .. } | Insn::Casp { rs, rt, rn, .. } => {
                g(rs);
                g(rt);
                g(rn)
            }
            Insn::Cbz { rt, .. } | Insn::Tbz { rt, .. } | Insn::Mrs { rt, .. } | Insn::Msr { rt, .. } => g(rt),
            Insn::Br { rn } | Insn::Blr { rn } | Insn::Ret { rn } => g(rn),
            Insn::Bl { .. } => g(30),
            _ => {}
        }
    }


    pub fn vreg_uses(&self, f: &mut dyn FnMut(u8)) {
        match *self {
            Insn::VLd { rt, .. } | Insn::VSt { rt, .. } => f(rt),
            Insn::VLdp { rt, rt2, .. } | Insn::VStp { rt, rt2, .. } => {
                f(rt);
                f(rt2)
            }
            Insn::Movi { rd, .. } | Insn::FmovI { rd, .. } | Insn::FmovFromG { rd, .. } | Insn::VDupG { rd, .. } | Insn::InsG { rd, .. } | Insn::CvtIF { rd, .. } => f(rd),
            Insn::FmovToG { rn, .. } | Insn::Umov { rn, .. } | Insn::CvtFI { rn, .. } => f(rn),
            Insn::FmovR { rd, rn, .. } | Insn::Fp1 { rd, rn, .. } | Insn::Fcvt { rd, rn, .. } | Insn::VDupE { rd, rn, .. } => {
                f(rd);
                f(rn)
            }
            Insn::Fp2 { rd, rn, rm, .. } | Insn::Fcsel { rd, rn, rm, .. } | Insn::VLog { rd, rn, rm, .. } | Insn::VArith { rd, rn, rm, .. } => {
                f(rd);
                f(rn);
                f(rm)
            }
            Insn::Fp3 { rd, rn, rm, ra, .. } => {
                f(rd);
                f(rn);
                f(rm);
                f(ra)
            }
            Insn::Fcmp { rn, rm, zero, .. } => {
                f(rn);
                if !zero {
                    f(rm)
                }
            }
            _ => {}
        }
    }

    pub fn is_term(&self) -> bool {
        matches!(
            self,
            Insn::B { .. }
                | Insn::Bl { .. }
                | Insn::Bcond { .. }
                | Insn::Cbz { .. }
                | Insn::Tbz { .. }
                | Insn::Br { .. }
                | Insn::Blr { .. }
                | Insn::Ret { .. }
                | Insn::Eret
                | Insn::Exc { .. }
        )
    }

    pub fn name(&self) -> &'static str {
        match self {
            Insn::Adr { .. } => "adr",
            Insn::Adrp { .. } => "adrp",
            Insn::AddSubImm { .. } => "addsub-imm",
            Insn::AddSubSh { .. } => "addsub-shift",
            Insn::AddSubExt { .. } => "addsub-ext",
            Insn::AddSubC { .. } => "adc-sbc",
            Insn::LogImm { .. } => "logic-imm",
            Insn::LogSh { .. } => "logic-shift",
            Insn::MovW { .. } => "movw",
            Insn::Bfm { .. } => "bitfield",
            Insn::Extr { .. } => "extr",
            Insn::ShV { .. } => "shift-var",
            Insn::Madd { .. } => "madd",
            Insn::Mull { .. } => "mull",
            Insn::Mulh { .. } => "mulh",
            Insn::Div { .. } => "div",
            Insn::Csel { .. } => "csel",
            Insn::Ccmp { .. } => "ccmp",
            Insn::Clz { .. } => "clz",
            Insn::Cls { .. } => "cls",
            Insn::Rbit { .. } => "rbit",
            Insn::Rev { .. } => "rev",
            Insn::Crc { .. } => "crc",
            Insn::Ld { .. } => "ld",
            Insn::St { .. } => "st",
            Insn::Ldp { .. } => "ldp",
            Insn::Stp { .. } => "stp",
            Insn::Ldex { .. } => "ldex",
            Insn::Stex { .. } => "stex",
            Insn::Ldar { .. } => "ldar",
            Insn::Stlr { .. } => "stlr",
            Insn::Atomic { .. } => "atomic",
            Insn::Cas { .. } => "cas",
            Insn::Casp { .. } => "casp",
            Insn::Prfm => "prfm",
            Insn::B { .. } => "b",
            Insn::Bl { .. } => "bl",
            Insn::Bcond { .. } => "bcond",
            Insn::Cbz { .. } => "cbz",
            Insn::Tbz { .. } => "tbz",
            Insn::Br { .. } => "br",
            Insn::Blr { .. } => "blr",
            Insn::Ret { .. } => "ret",
            Insn::Eret => "eret",
            Insn::Hint { .. } => "hint",
            Insn::Pac { .. } => "pac",
            Insn::Barrier { .. } => "barrier",
            Insn::Mrs { .. } => "mrs",
            Insn::Msr { .. } => "msr",
            Insn::MsrImm { .. } => "msr-imm",
            Insn::Sys { .. } => "sys",
            Insn::Exc { .. } => "exc",
            Insn::Unsup(s) => s,
            _ => "simd-fp",
        }
    }
}
