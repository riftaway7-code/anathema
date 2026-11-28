use alloc::vec::Vec;
pub const RAX: u8 = 0;
pub const RCX: u8 = 1;
pub const RDX: u8 = 2;
pub const RBX: u8 = 3;
pub const RSP: u8 = 4;
pub const RBP: u8 = 5;
pub const RSI: u8 = 6;
pub const RDI: u8 = 7;
pub const R8: u8 = 8;
pub const R9: u8 = 9;
pub const R10: u8 = 10;
pub const R11: u8 = 11;
pub const R12: u8 = 12;
pub const R13: u8 = 13;
pub const R14: u8 = 14;
pub const R15: u8 = 15;

pub const NOIDX: u8 = 0xff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mem {
    pub base: u8,
    pub index: u8,
    pub scale: u8,
    pub disp: i32,
}

impl Mem {
    pub fn b(base: u8, disp: i32) -> Mem {
        Mem { base, index: NOIDX, scale: 0, disp }
    }
    pub fn bi(base: u8, index: u8, scale: u8, disp: i32) -> Mem {
        Mem { base, index, scale, disp }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rm {
    R(u8),
    M(Mem),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alu {
    Add = 0,
    Or = 1,
    Adc = 2,
    Sbb = 3,
    And = 4,
    Sub = 5,
    Xor = 6,
    Cmp = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shf {
    Rol = 0,
    Ror = 1,
    Shl = 4,
    Shr = 5,
    Sar = 7,
}

pub const CC_O: u8 = 0;
pub const CC_NO: u8 = 1;
pub const CC_B: u8 = 2;
pub const CC_AE: u8 = 3;
pub const CC_E: u8 = 4;
pub const CC_NE: u8 = 5;
pub const CC_BE: u8 = 6;
pub const CC_A: u8 = 7;
pub const CC_P: u8 = 10;
pub const CC_S: u8 = 8;
pub const CC_NS: u8 = 9;
pub const CC_L: u8 = 12;
pub const CC_GE: u8 = 13;
pub const CC_LE: u8 = 14;
pub const CC_G: u8 = 15;

#[derive(Default)]
pub struct Asm {
    pub hot: Vec<u8>,
    pub cold: Vec<u8>,
    pub in_cold: bool,
}

impl Asm {
    pub fn new() -> Asm {
        Asm { hot: Vec::with_capacity(1024), cold: Vec::with_capacity(512), in_cold: false }
    }

    #[inline]
    pub fn buf(&mut self) -> &mut Vec<u8> {
        if self.in_cold {
            &mut self.cold
        } else {
            &mut self.hot
        }
    }

    #[inline]
    pub fn pos(&self) -> usize {
        if self.in_cold {
            self.cold.len()
        } else {
            self.hot.len()
        }
    }

    #[inline]
    pub fn b(&mut self, v: u8) {
        self.buf().push(v);
    }

    pub fn d32(&mut self, v: i32) {
        self.buf().extend_from_slice(&v.to_le_bytes());
    }

    pub fn d64(&mut self, v: u64) {
        self.buf().extend_from_slice(&v.to_le_bytes());
    }

    fn rex(&mut self, w: bool, reg: u8, idx: u8, base: u8, force: bool) {
        let r = 0x40 | ((w as u8) << 3) | (((reg >> 3) & 1) << 2) | (((idx >> 3) & 1) << 1) | ((base >> 3) & 1);
        if r != 0x40 || force {
            self.b(r);
        }
    }

    fn modrm(&mut self, reg: u8, rm: Rm) {
        match rm {
            Rm::R(r) => self.b(0xc0 | ((reg & 7) << 3) | (r & 7)),
            Rm::M(m) if m.base == NOIDX => {
                self.b(((reg & 7) << 3) | 4);
                self.b((m.scale << 6) | ((m.index & 7) << 3) | 5);
                self.d32(m.disp);
            }
            Rm::M(m) => {
                let d = m.disp;
                let need_sib = m.index != NOIDX || (m.base & 7) == 4;
                let md = if d == 0 && (m.base & 7) != 5 {
                    0
                } else if (-128..=127).contains(&d) {
                    1
                } else {
                    2
                };
                if need_sib {
                    self.b((md << 6) | ((reg & 7) << 3) | 4);
                    let idx = if m.index == NOIDX { 4 } else { m.index & 7 };
                    self.b((m.scale << 6) | (idx << 3) | (m.base & 7));
                } else {
                    self.b((md << 6) | ((reg & 7) << 3) | (m.base & 7));
                }
                match md {
                    1 => self.b(d as i8 as u8),
                    2 => self.d32(d),
                    _ => {}
                }
            }
        }
    }

    pub fn op(&mut self, pfx: u8, w: bool, opc: &[u8], reg: u8, rm: Rm, force_rex: bool) {
        if pfx != 0 {
            self.b(pfx);
        }
        let (idx, base) = match rm {
            Rm::R(r) => (0, r),
            Rm::M(m) => (if m.index == NOIDX { 0 } else { m.index }, if m.base == NOIDX { 0 } else { m.base }),
        };
        self.rex(w, reg, idx, base, force_rex);
        for &o in opc {
            self.b(o);
        }
        self.modrm(reg, rm);
    }

    pub fn vex(&mut self, pp: u8, map: u8, w: bool, vvvv: u8, reg: u8, rm: Rm, opc: u8) {
        let (idx, base) = match rm {
            Rm::R(r) => (0, r),
            Rm::M(m) => (if m.index == NOIDX { 0 } else { m.index }, if m.base == NOIDX { 0 } else { m.base }),
        };
        if map == 1 && !w && idx < 8 && base < 8 {
            self.b(0xc5);
            self.b((((!(reg >> 3)) & 1) << 7) | (((!vvvv) & 0xf) << 3) | pp);
        } else {
            self.b(0xc4);
            self.b((((!(reg >> 3)) & 1) << 7) | (((!(idx >> 3)) & 1) << 6) | (((!(base >> 3)) & 1) << 5) | map);
            self.b(((w as u8) << 7) | (((!vvvv) & 0xf) << 3) | pp);
        }
        self.b(opc);
        self.modrm(reg, rm);
    }

    pub fn alu_rr(&mut self, op: Alu, w: bool, d: u8, s: u8) {
        self.op(0, w, &[(op as u8) * 8 + 1], s, Rm::R(d), false);
    }

    pub fn alu_rm(&mut self, op: Alu, w: bool, d: u8, m: Mem) {
        self.op(0, w, &[(op as u8) * 8 + 3], d, Rm::M(m), false);
    }

    pub fn alu_mr(&mut self, op: Alu, w: bool, m: Mem, s: u8) {
        self.op(0, w, &[(op as u8) * 8 + 1], s, Rm::M(m), false);
    }

    pub fn alu_ri(&mut self, op: Alu, w: bool, d: u8, imm: i32) {
        self.alu_xi(op, w, Rm::R(d), imm);
    }

    pub fn alu_mi(&mut self, op: Alu, w: bool, m: Mem, imm: i32) {
        self.alu_xi(op, w, Rm::M(m), imm);
    }

    fn alu_xi(&mut self, op: Alu, w: bool, rm: Rm, imm: i32) {
        if (-128..=127).contains(&imm) {
            self.op(0, w, &[0x83], op as u8, rm, false);
            self.b(imm as i8 as u8);
        } else {
            self.op(0, w, &[0x81], op as u8, rm, false);
            self.d32(imm);
        }
    }

    pub fn test_rr(&mut self, w: bool, a: u8, b: u8) {
        self.op(0, w, &[0x85], b, Rm::R(a), false);
    }

    pub fn test_ri(&mut self, w: bool, a: u8, imm: i32) {
        self.op(0, w, &[0xf7], 0, Rm::R(a), false);
        self.d32(imm);
    }

    pub fn mov_rr(&mut self, w: bool, d: u8, s: u8) {
        self.op(0, w, &[0x89], s, Rm::R(d), false);
    }

    pub fn mov_ri(&mut self, d: u8, v: u64) {
        if v <= u32::MAX as u64 {
            self.rex(false, 0, 0, d, false);
            self.b(0xb8 | (d & 7));
            self.d32(v as u32 as i32);
        } else if (v as i64) >= i32::MIN as i64 && (v as i64) < 0 {
            self.op(0, true, &[0xc7], 0, Rm::R(d), false);
            self.d32(v as i32);
        } else {
            self.rex(true, 0, 0, d, false);
            self.b(0xb8 | (d & 7));
            self.d64(v);
        }
    }

    pub fn mov_ri64(&mut self, d: u8, v: u64) {
        self.rex(true, 0, 0, d, false);
        self.b(0xb8 | (d & 7));
        self.d64(v);
    }

    pub fn load(&mut self, size: u8, signed: bool, w64: bool, d: u8, m: Mem) {
        let rm = Rm::M(m);
        match (size, signed) {
            (0, false) => self.op(0, false, &[0x0f, 0xb6], d, rm, false),
            (1, false) => self.op(0, false, &[0x0f, 0xb7], d, rm, false),
            (2, false) => self.op(0, false, &[0x8b], d, rm, false),
            (3, _) => self.op(0, true, &[0x8b], d, rm, false),
            (0, true) => self.op(0, w64, &[0x0f, 0xbe], d, rm, false),
            (1, true) => self.op(0, w64, &[0x0f, 0xbf], d, rm, false),
            (2, true) => {
                if w64 {
                    self.op(0, true, &[0x63], d, rm, false)
                } else {
                    self.op(0, false, &[0x8b], d, rm, false)
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn store(&mut self, size: u8, m: Mem, s: u8) {
        let rm = Rm::M(m);
        match size {
            0 => self.op(0, false, &[0x88], s, rm, (4..8).contains(&s)),
            1 => self.op(0x66, false, &[0x89], s, rm, false),
            2 => self.op(0, false, &[0x89], s, rm, false),
            _ => self.op(0, true, &[0x89], s, rm, false),
        }
    }

    pub fn store_i(&mut self, size: u8, m: Mem, imm: i32) {
        let rm = Rm::M(m);
        match size {
            0 => {
                self.op(0, false, &[0xc6], 0, rm, false);
                self.b(imm as u8);
            }
            1 => {
                self.op(0x66, false, &[0xc7], 0, rm, false);
                self.buf().extend_from_slice(&(imm as u16).to_le_bytes());
            }
            2 => {
                self.op(0, false, &[0xc7], 0, rm, false);
                self.d32(imm);
            }
            _ => {
                self.op(0, true, &[0xc7], 0, rm, false);
                self.d32(imm);
            }
        }
    }

    pub fn movzx_rr(&mut self, size: u8, d: u8, s: u8) {
        match size {
            0 => self.op(0, false, &[0x0f, 0xb6], d, Rm::R(s), (4..8).contains(&s)),
            1 => self.op(0, false, &[0x0f, 0xb7], d, Rm::R(s), false),
            _ => self.mov_rr(false, d, s),
        }
    }

    pub fn movsx_rr(&mut self, size: u8, w64: bool, d: u8, s: u8) {
        match size {
            0 => self.op(0, w64, &[0x0f, 0xbe], d, Rm::R(s), (4..8).contains(&s)),
            1 => self.op(0, w64, &[0x0f, 0xbf], d, Rm::R(s), false),
            _ => {
                if w64 {
                    self.op(0, true, &[0x63], d, Rm::R(s), false)
                } else {
                    self.mov_rr(false, d, s)
                }
            }
        }
    }

    pub fn lea(&mut self, w: bool, d: u8, m: Mem) {
        self.op(0, w, &[0x8d], d, Rm::M(m), false);
    }

    pub fn shift_i(&mut self, k: Shf, w: bool, d: u8, imm: u8) {
        if imm == 1 {
            self.op(0, w, &[0xd1], k as u8, Rm::R(d), false);
        } else {
            self.op(0, w, &[0xc1], k as u8, Rm::R(d), false);
            self.b(imm);
        }
    }

    pub fn shift_cl(&mut self, k: Shf, w: bool, d: u8) {
        self.op(0, w, &[0xd3], k as u8, Rm::R(d), false);
    }

    pub fn neg(&mut self, w: bool, d: u8) {
        self.op(0, w, &[0xf7], 3, Rm::R(d), false);
    }

    pub fn not(&mut self, w: bool, d: u8) {
        self.op(0, w, &[0xf7], 2, Rm::R(d), false);
    }

    pub fn imul_rr(&mut self, w: bool, d: u8, s: u8) {
        self.op(0, w, &[0x0f, 0xaf], d, Rm::R(s), false);
    }

    pub fn imul_rm(&mut self, w: bool, d: u8, m: Mem) {
        self.op(0, w, &[0x0f, 0xaf], d, Rm::M(m), false);
    }

    pub fn grp3(&mut self, w: bool, ext: u8, rm: Rm) {
        self.op(0, w, &[0xf7], ext, rm, false);
    }

    pub fn cqo(&mut self, w: bool) {
        if w {
            self.b(0x48);
        }
        self.b(0x99);
    }

    pub fn bswap(&mut self, w: bool, d: u8) {
        self.rex(w, 0, 0, d, false);
        self.b(0x0f);
        self.b(0xc8 | (d & 7));
    }

    pub fn lzcnt(&mut self, w: bool, d: u8, s: u8) {
        self.op(0xf3, w, &[0x0f, 0xbd], d, Rm::R(s), false);
    }

    pub fn tzcnt(&mut self, w: bool, d: u8, s: u8) {
        self.op(0xf3, w, &[0x0f, 0xbc], d, Rm::R(s), false);
    }

    pub fn setcc(&mut self, cc: u8, d: u8) {
        self.op(0, false, &[0x0f, 0x90 | cc], 0, Rm::R(d), (4..8).contains(&d));
    }

    pub fn cmovcc(&mut self, cc: u8, w: bool, d: u8, s: u8) {
        self.op(0, w, &[0x0f, 0x40 | cc], d, Rm::R(s), false);
    }

    pub fn cmovcc_m(&mut self, cc: u8, w: bool, d: u8, m: Mem) {
        self.op(0, w, &[0x0f, 0x40 | cc], d, Rm::M(m), false);
    }

    pub fn jcc32(&mut self, cc: u8) -> usize {
        self.b(0x0f);
        self.b(0x80 | cc);
        let p = self.pos();
        self.d32(0);
        p
    }

    pub fn jmp32(&mut self) -> usize {
        self.b(0xe9);
        let p = self.pos();
        self.d32(0);
        p
    }

    pub fn jcc8(&mut self, cc: u8) -> usize {
        self.b(0x70 | cc);
        let p = self.pos();
        self.b(0);
        p
    }

    pub fn jmp8(&mut self) -> usize {
        self.b(0xeb);
        let p = self.pos();
        self.b(0);
        p
    }

    pub fn patch8(&mut self, p: usize) {
        let end = self.pos();
        let v = (end - p - 1) as i64;
        debug_assert!(v <= 127);
        self.buf()[p] = v as u8;
    }

    pub fn jmp_rm(&mut self, rm: Rm) {
        self.op(0, false, &[0xff], 4, rm, false);
    }

    pub fn call_rm(&mut self, rm: Rm) {
        self.op(0, false, &[0xff], 2, rm, false);
    }

    pub fn ret(&mut self) {
        self.b(0xc3);
    }

    pub fn push_r(&mut self, r: u8) {
        self.rex(false, 0, 0, r, false);
        self.b(0x50 | (r & 7));
    }

    pub fn pop_r(&mut self, r: u8) {
        self.rex(false, 0, 0, r, false);
        self.b(0x58 | (r & 7));
    }

    pub fn push_m(&mut self, m: Mem) {
        self.op(0, false, &[0xff], 6, Rm::M(m), false);
    }

    pub fn pop_m(&mut self, m: Mem) {
        self.op(0, false, &[0x8f], 0, Rm::M(m), false);
    }

    pub fn push_i32(&mut self, v: i32) {
        self.b(0x68);
        self.d32(v);
    }

    pub fn pushfq(&mut self) {
        self.b(0x9c);
    }

    pub fn popfq(&mut self) {
        self.b(0x9d);
    }

    pub fn cmc(&mut self) {
        self.b(0xf5);
    }

    pub fn mfence(&mut self) {
        self.buf().extend_from_slice(&[0x0f, 0xae, 0xf0]);
    }

    pub fn lock(&mut self) {
        self.b(0xf0);
    }

    pub fn xchg_mr(&mut self, size: u8, m: Mem, r: u8) {
        match size {
            0 => self.op(0, false, &[0x86], r, Rm::M(m), (4..8).contains(&r)),
            1 => self.op(0x66, false, &[0x87], r, Rm::M(m), false),
            2 => self.op(0, false, &[0x87], r, Rm::M(m), false),
            _ => self.op(0, true, &[0x87], r, Rm::M(m), false),
        }
    }

    pub fn cmpxchg(&mut self, size: u8, m: Mem, r: u8) {
        self.lock();
        match size {
            0 => self.op(0, false, &[0x0f, 0xb0], r, Rm::M(m), (4..8).contains(&r)),
            1 => self.op(0x66, false, &[0x0f, 0xb1], r, Rm::M(m), false),
            2 => self.op(0, false, &[0x0f, 0xb1], r, Rm::M(m), false),
            _ => self.op(0, true, &[0x0f, 0xb1], r, Rm::M(m), false),
        }
    }

    pub fn xadd(&mut self, size: u8, m: Mem, r: u8) {
        self.lock();
        match size {
            0 => self.op(0, false, &[0x0f, 0xc0], r, Rm::M(m), (4..8).contains(&r)),
            1 => self.op(0x66, false, &[0x0f, 0xc1], r, Rm::M(m), false),
            2 => self.op(0, false, &[0x0f, 0xc1], r, Rm::M(m), false),
            _ => self.op(0, true, &[0x0f, 0xc1], r, Rm::M(m), false),
        }
    }

    pub fn shlx(&mut self, w: bool, d: u8, s: Rm, cnt: u8) {
        self.vex(1, 2, w, cnt, d, s, 0xf7);
    }

    pub fn shrx(&mut self, w: bool, d: u8, s: Rm, cnt: u8) {
        self.vex(3, 2, w, cnt, d, s, 0xf7);
    }

    pub fn sarx(&mut self, w: bool, d: u8, s: Rm, cnt: u8) {
        self.vex(2, 2, w, cnt, d, s, 0xf7);
    }

    pub fn rorx(&mut self, w: bool, d: u8, s: Rm, imm: u8) {
        self.vex(3, 3, w, 0, d, s, 0xf0);
        self.b(imm);
    }

    pub fn andn(&mut self, w: bool, d: u8, inv: u8, s: Rm) {
        self.vex(0, 2, w, inv, d, s, 0xf2);
    }

    pub fn mulx(&mut self, w: bool, hi: u8, lo: u8, s: Rm) {
        self.vex(3, 2, w, lo, hi, s, 0xf6);
    }

    pub fn bzhi(&mut self, w: bool, d: u8, s: Rm, idx: u8) {
        self.vex(0, 2, w, idx, d, s, 0xf5);
    }

    pub fn shld_i(&mut self, w: bool, d: u8, s: u8, imm: u8) {
        self.op(0, w, &[0x0f, 0xa4], s, Rm::R(d), false);
        self.b(imm);
    }

    pub fn shrd_i(&mut self, w: bool, d: u8, s: u8, imm: u8) {
        self.op(0, w, &[0x0f, 0xac], s, Rm::R(d), false);
        self.b(imm);
    }

    pub fn crc32(&mut self, size: u8, d: u8, s: u8) {
        match size {
            0 => self.op(0xf2, false, &[0x0f, 0x38, 0xf0], d, Rm::R(s), (4..8).contains(&s)),
            1 => {
                self.b(0x66);
                self.op(0xf2, false, &[0x0f, 0x38, 0xf1], d, Rm::R(s), false)
            }
            2 => self.op(0xf2, false, &[0x0f, 0x38, 0xf1], d, Rm::R(s), false),
            _ => self.op(0xf2, true, &[0x0f, 0x38, 0xf1], d, Rm::R(s), false),
        }
    }

    pub fn bt_ri(&mut self, w: bool, r: u8, bit: u8) {
        self.op(0, w, &[0x0f, 0xba], 4, Rm::R(r), false);
        self.b(bit);
    }

    pub fn ud2(&mut self) {
        self.buf().extend_from_slice(&[0x0f, 0x0b]);
    }
}
