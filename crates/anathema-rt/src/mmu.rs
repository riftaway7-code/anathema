use alloc::string::String;
use crate::mem::Ram;

#[derive(Default, Clone)]
pub struct Mmu {
    pub sctlr: u64,
    pub tcr: u64,
    pub ttbr0: u64,
    pub ttbr1: u64,
}

pub struct Fault {
    pub va: u64,
    pub level: u32,
    pub kind: u8,
}

impl Mmu {
    pub fn walk_debug(&self, ram: &Ram, va: u64) -> String {
        let upper = (va >> 55) & 1 == 1;
        let (tsz, ttbr) = if upper { ((self.tcr >> 16) & 0x3f, self.ttbr1) } else { (self.tcr & 0x3f, self.ttbr0) };
        let va_bits = 64 - tsz as u32;
        let mut table = ttbr & 0x0000_ffff_ffff_fffe & !0x3f;
        let mut out = format!("walk {:#x} ttbr {:#x}
", va, table);
        let mut shift = 36u32;
        for level in 1..4 {
            let idx_bits = if shift == 36 { va_bits - 36 } else { 11 };
            let idx = (va >> shift) & ((1u64 << idx_bits) - 1);
            let at = table + idx * 8;
            if !ram.contains(at, 8) {
                out += &format!(" L{} at {:#x} outside ram
", level, at);
                break;
            }
            let desc = ram.read(at, 8);
            out += &format!(" L{} idx {} at {:#x} = {:#x}
", level, idx, at, desc);
            if desc & 3 != 3 || level == 3 {
                break;
            }
            table = desc & 0x0000_ffff_ffff_c000;
            shift -= 11;
        }
        out
    }

    pub fn enabled(&self) -> bool {
        self.sctlr & 1 != 0
    }

    pub fn translate(&self, ram: &Ram, va: u64) -> Result<(u64, u8), Fault> {
        if !self.enabled() {
            return Ok((va, 0x0b));
        }
        let upper = (va >> 55) & 1 == 1;
        let (tsz, tg, ttbr) = if upper {
            ((self.tcr >> 16) & 0x3f, (self.tcr >> 30) & 3, self.ttbr1)
        } else {
            (self.tcr & 0x3f, (self.tcr >> 14) & 3, self.ttbr0)
        };
        let grain_shift = match (upper, tg) {
            (false, 0) | (true, 2) => 12,
            (false, 2) | (true, 1) => 14,
            _ => 16,
        };
        let bits_per_level = grain_shift - 3;
        let va_bits = 64 - tsz as u32;
        let levels = (va_bits - grain_shift + bits_per_level - 1) / bits_per_level;
        let top_shift = grain_shift + bits_per_level * (levels - 1);
        let mut table = ttbr & 0x0000_ffff_ffff_fffe & !0x3f;
        let mut shift = top_shift;
        for level in (4 - levels)..4 {
            let idx_bits = if shift == top_shift { va_bits - top_shift } else { bits_per_level };
            let idx = (va >> shift) & ((1u64 << idx_bits) - 1);
            let at = table + idx * 8;
            if !ram.contains(at, 8) {
                return Err(Fault { va, level, kind: 0 });
            }
            let desc = ram.read(at, 8);
            if desc & 1 == 0 {
                return Err(Fault { va, level, kind: 0 });
            }
            let is_table = desc & 2 != 0;
            let leaf_flags = |desc: u64, level: u32| -> u8 {
                let user = ((desc >> 6) & 1) as u8;
                let ro = ((desc >> 7) & 1) as u8;
                let uxn = ((desc >> 54) & 1) as u8;
                let pxn = ((desc >> 53) & 1) as u8;
                user | (ro << 1) | (uxn << 2) | (pxn << 3) | ((level as u8) << 4)
            };
            if level == 3 {
                if !is_table {
                    return Err(Fault { va, level, kind: 0 });
                }
                if desc & (1 << 10) == 0 {
                    return Err(Fault { va, level, kind: 1 });
                }
                let mask = !((1u64 << grain_shift) - 1) & 0x0000_ffff_ffff_ffff;
                return Ok(((desc & mask) | (va & ((1u64 << grain_shift) - 1)), leaf_flags(desc, level)));
            }
            if !is_table {
                if desc & (1 << 10) == 0 {
                    return Err(Fault { va, level, kind: 1 });
                }
                let mask = !((1u64 << shift) - 1) & 0x0000_ffff_ffff_ffff;
                return Ok(((desc & mask) | (va & ((1u64 << shift) - 1)), leaf_flags(desc, level)));
            }
            table = desc & 0x0000_ffff_ffff_ffff & !((1u64 << grain_shift) - 1);
            shift -= bits_per_level;
        }
        Err(Fault { va, level: 3, kind: 0 })
    }
}
