use alloc::{string::String, vec::Vec};
use anathema_xlat::run::Region;
use anathema_xlat::fx::HashMap;

pub const RAM_BASE: u64 = 0x7000_0000;
pub const RAM_SIZE: u64 = 0x1_0000_0000;

pub struct Ram {
    pub base: u64,
    pub len: u64,
    host: *mut u8,
    _region: Option<Region>,
}

impl Ram {
    #[cfg(feature = "std")]
    pub fn new(base: u64, len: u64) -> Ram {
        let region = Region::anon(len as usize);
        Ram { base, len, host: region.ptr, _region: Some(region) }
    }

    pub unsafe fn from_raw(base: u64, len: u64, host: *mut u8) -> Ram {
        Ram { base, len, host, _region: None }
    }

    fn p(&self, pa: u64) -> *mut u8 {
        unsafe { self.host.add(pa.wrapping_sub(self.base) as usize) }
    }

    pub fn contains(&self, pa: u64, n: u64) -> bool {
        pa >= self.base && pa.wrapping_add(n) <= self.base + self.len && pa.wrapping_add(n) >= pa
    }

    pub fn read(&self, pa: u64, size: usize) -> u64 {
        let p = self.p(pa) as *const u8;
        unsafe {
            match size {
                1 => *p as u64,
                2 => (p as *const u16).read_unaligned() as u64,
                4 => (p as *const u32).read_unaligned() as u64,
                _ => (p as *const u64).read_unaligned(),
            }
        }
    }

    pub fn write(&self, pa: u64, size: usize, v: u64) {
        let p = self.p(pa);
        unsafe {
            match size {
                1 => *p = v as u8,
                2 => (p as *mut u16).write_unaligned(v as u16),
                4 => (p as *mut u32).write_unaligned(v as u32),
                _ => (p as *mut u64).write_unaligned(v),
            }
        }
    }

    pub fn bytes(&self, pa: u64, len: usize) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.p(pa), len) }
    }
}

pub const FB_BASE: u64 = 0x6000_0000;
pub const FB_BYTES: usize = 1920 * 1080 * 4;

pub struct Bus {
    pub gic: crate::gic::Gic,
    pub fb: Vec<u8>,
    pub uart: String,
    regs: HashMap<u64, u64>,
    pub aes_irq_status: u32,
    pub aes_irq_enable: u32,
    pub aes_flag_info: u32,
    pub aes_fifo: Vec<u32>,
    pub aes_kick: bool,
    pub log_budget: usize,
    pub cur_pc: u64,
    pub gfx_kick: bool,
    pub gfx_log: usize,
    pub seen: HashMap<u64, usize>,
}

impl Bus {
    pub fn new() -> Bus {
        Bus { gic: crate::gic::Gic::new(), fb: vec![0u8; FB_BYTES], uart: String::new(), regs: HashMap::default(), aes_irq_status: 0, aes_irq_enable: 0, aes_flag_info: 0, aes_fifo: Vec::new(), aes_kick: false, log_budget: 300, cur_pc: 0, gfx_kick: false, gfx_log: 400, seen: HashMap::default() }
    }

    fn note(&mut self, kind: &str, pa: u64, size: usize, v: u64) {
        if (0x2001_0000..0x2001_1000).contains(&pa) {
            return;
        }
        if (0x3020_0000..0x3022_0000).contains(&pa) && self.gfx_log > 0 {
            self.gfx_log -= 1;
            println!("  gfx {} {:#x} size {} = {:#x} pc {:#x}", kind, pa, size, v, self.cur_pc);
        }
        let region = pa & !0xffff;
        let n = self.seen.entry(region).or_insert(0);
        *n += 1;
        if self.log_budget > 0 && *n <= 40 {
            self.log_budget -= 1;
            println!("  mmio {} {:#x} size {} = {:#x}", kind, pa, size, v);
        }
    }

    pub fn read(&mut self, pa: u64, size: usize) -> u64 {
        if pa >= FB_BASE && pa + size as u64 <= FB_BASE + FB_BYTES as u64 {
            let o = (pa - FB_BASE) as usize;
            let mut b = [0u8; 8];
            b[..size].copy_from_slice(&self.fb[o..o + size]);
            return u64::from_le_bytes(b);
        }
        if pa >= crate::gic::GIC_BASE && pa < crate::gic::GIC_BASE + crate::gic::GICD_SIZE {
            return self.gic.dist((pa - crate::gic::GIC_BASE) as usize, size, None);
        }
        if pa >= crate::gic::MSI_BASE && pa < crate::gic::MSI_BASE + 0x1_0000 {
            return self.gic.msi((pa - crate::gic::MSI_BASE) as usize, None);
        }
        if pa >= crate::gic::GICR_BASE && pa < crate::gic::GICR_BASE + crate::gic::GICR_SIZE {
            return self.gic.redist((pa - crate::gic::GICR_BASE) as usize, size, None);
        }
        if (0x3022_0000..0x3022_4000).contains(&pa) {
            let off = pa - 0x3022_0000;
            return match off {
                0xc => 0x3f80,
                0x18 => self.aes_irq_status as u64,
                0x1c => self.aes_irq_enable as u64,
                0x24 => 2,
                0x30 => self.aes_flag_info as u64,
                _ => 0,
            };
        }
        if (0x3023_0000..0x3023_4000).contains(&pa) {
            return 0;
        }
        if pa >= 0x4000_0000 && pa < 0x5000_0000 {
            return if size >= 8 { u64::MAX } else { (1u64 << (size * 8)) - 1 };
        }
        let v = match pa {
            0x2001_0018 => 0x90,
            0x2001_0fe0 => 0x11,
            0x2001_0fe4 => 0x10,
            0x2001_0fe8 => 0x34,
            0x2001_0fec => 0x00,
            0x2001_0ff0 => 0x0d,
            0x2001_0ff4 => 0xf0,
            0x2001_0ff8 => 0x05,
            0x2001_0ffc => 0xb1,
            _ => self.regs.get(&pa).copied().unwrap_or(0),
        };
        self.note("r", pa, size, v);
        v
    }

    pub fn reg(&self, pa: u64) -> u64 {
        self.regs.get(&pa).copied().unwrap_or(0)
    }

    pub fn write(&mut self, pa: u64, size: usize, v: u64) {
        if pa == 0x3020_1008 {
            self.gfx_kick = true;
        }
        if pa >= FB_BASE && pa + size as u64 <= FB_BASE + FB_BYTES as u64 {
            let o = (pa - FB_BASE) as usize;
            self.fb[o..o + size].copy_from_slice(&v.to_le_bytes()[..size]);
            return;
        }
        if pa >= crate::gic::GIC_BASE && pa < crate::gic::GIC_BASE + crate::gic::GICD_SIZE {
            self.gic.dist((pa - crate::gic::GIC_BASE) as usize, size, Some(v));
            return;
        }
        if pa >= crate::gic::MSI_BASE && pa < crate::gic::MSI_BASE + 0x1_0000 {
            self.gic.msi((pa - crate::gic::MSI_BASE) as usize, Some(v));
            return;
        }
        if pa >= crate::gic::GICR_BASE && pa < crate::gic::GICR_BASE + crate::gic::GICR_SIZE {
            self.gic.redist((pa - crate::gic::GICR_BASE) as usize, size, Some(v));
            return;
        }
        if (0x3022_0000..0x3022_4000).contains(&pa) {
            let off = pa - 0x3022_0000;
            match off {
                0x18 => self.aes_irq_status &= !(v as u32),
                0x1c => self.aes_irq_enable = v as u32,
                0x200 => self.aes_fifo.push(v as u32),
                _ => {}
            }
            self.aes_kick = true;
            return;
        }
        if pa == 0x2001_0000 {
            let c = (v & 0xff) as u8 as char;
            self.uart.push(c);
            print!("{}", c);
            return;
        }
        self.regs.insert(pa, v);
        self.note("w", pa, size, v);
    }
}
