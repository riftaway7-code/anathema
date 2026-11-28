use alloc::vec::Vec;
pub const GIC_BASE: u64 = 0x1000_0000;
pub const GICD_SIZE: u64 = 0x1_0000;
pub const GICR_BASE: u64 = 0x1001_0000;
pub const GICR_SIZE: u64 = 0x40_0000;
pub const NINT: usize = 512;
pub const NCPU: usize = 1;
pub const MSI_BASE: u64 = 0x1fff_0000;
pub const MSI_SPI_BASE: u32 = 256;
pub const MSI_SPI_COUNT: u32 = 128;

pub struct Gic {
    pub dctl: u32,
    bits: [Vec<bool>; 4],
    prio: Vec<u8>,
    cfg: Vec<u32>,
    route: Vec<u64>,
    waker: u32,
    pub sre: u64,
    pub pmr: u64,
    pub bpr0: u64,
    pub bpr1: u64,
    pub ctlr: u64,
    pub igrpen0: u64,
    pub igrpen1: u64,
    pub ap: [u64; 8],
    running: Vec<(u8, usize)>,
    pub level: Vec<bool>,
}

fn pidr2(off: usize) -> Option<u64> {
    if off == 0xffe8 {
        Some(0x3b)
    } else {
        None
    }
}

impl Gic {
    pub fn new() -> Gic {
        Gic {
            dctl: 0,
            bits: [vec![false; NINT], vec![false; NINT], vec![false; NINT], vec![false; NINT]],
            prio: vec![0; NINT],
            cfg: vec![0; NINT / 16],
            route: vec![0; NINT],
            waker: 0,
            sre: 7,
            pmr: 0xff,
            bpr0: 0,
            bpr1: 0,
            ctlr: 0x400,
            igrpen0: 1,
            igrpen1: 1,
            ap: [0; 8],
            running: Vec::new(),
            level: vec![false; NINT],
        }
    }

    fn word_r(&self, bank: usize, idx: usize) -> u32 {
        let mut r = 0u32;
        for i in 0..32 {
            if idx * 32 + i < NINT && self.bits[bank][idx * 32 + i] {
                r |= 1 << i;
            }
        }
        r
    }

    fn word_w(&mut self, bank: usize, idx: usize, v: u32, mode: u8) {
        for i in 0..32 {
            let n = idx * 32 + i;
            if n >= NINT {
                break;
            }
            let b = v >> i & 1 == 1;
            match mode {
                0 => self.bits[bank][n] = b,
                1 => {
                    if b {
                        self.bits[bank][n] = true
                    }
                }
                _ => {
                    if b {
                        self.bits[bank][n] = false
                    }
                }
            }
        }
    }

    fn common(&mut self, off: usize, size: usize, w: Option<u64>) -> u64 {
        let words = (size / 4).max(1);
        let bank_rw = |s: &mut Gic, bank: usize, base: usize, mode: u8| -> u64 {
            let mut r = 0u64;
            for k in 0..words {
                let idx = (off - base) / 4 + k;
                r |= (s.word_r(bank, idx) as u64) << (32 * k);
                if let Some(v) = w {
                    s.word_w(bank, idx, (v >> (32 * k)) as u32, mode);
                }
            }
            r
        };
        match off {
            0x80..=0xff => bank_rw(self, 0, 0x80, 0),
            0x100..=0x17f => bank_rw(self, 1, 0x100, 1),
            0x180..=0x1ff => bank_rw(self, 1, 0x180, 2),
            0x200..=0x27f => bank_rw(self, 2, 0x200, 1),
            0x280..=0x2ff => bank_rw(self, 2, 0x280, 2),
            0x300..=0x37f => bank_rw(self, 3, 0x300, 1),
            0x380..=0x3ff => bank_rw(self, 3, 0x380, 2),
            0x400..=0x7ff => {
                let mut r = 0u64;
                for k in 0..size {
                    let n = off - 0x400 + k;
                    if n < NINT {
                        r |= (self.prio[n] as u64) << (8 * k);
                        if let Some(v) = w {
                            self.prio[n] = (v >> (8 * k)) as u8;
                        }
                    }
                }
                r
            }
            0xc00..=0xcff => {
                let mut r = 0u64;
                for k in 0..words {
                    let idx = (off - 0xc00) / 4 + k;
                    if idx < self.cfg.len() {
                        r |= (self.cfg[idx] as u64) << (32 * k);
                        if let Some(v) = w {
                            self.cfg[idx] = (v >> (32 * k)) as u32;
                        }
                    }
                }
                r
            }
            _ => 0,
        }
    }

    pub fn msi(&mut self, off: usize, w: Option<u64>) -> u64 {
        match off {
            0x8 => (1u64 << 31) | (MSI_SPI_BASE as u64) << 16 | MSI_SPI_COUNT as u64,
            0x40 => {
                if let Some(v) = w {
                    let id = (v & 0xffff) as usize;
                    if id < NINT {
                        self.bits[2][id] = true;
                    }
                }
                0
            }
            0xfe8 => 0x3b,
            _ => 0,
        }
    }

    pub fn dist(&mut self, off: usize, size: usize, w: Option<u64>) -> u64 {
        if let Some(v) = pidr2(off) {
            return v;
        }
        match off {
            0x0 => {
                let r = self.dctl as u64 & 0x7fff_ffff;
                if let Some(v) = w {
                    self.dctl = v as u32 & 0x7fff_ffff;
                }
                r
            }
            0x4 => 15 | (15 << 19) | (1 << 24),
            0x8 => 0x0100_043b,
            0x6000..=0x7fff => {
                let n = (off - 0x6000) / 8;
                if n < NINT {
                    let r = self.route[n];
                    if let Some(v) = w {
                        self.route[n] = v;
                    }
                    r
                } else {
                    0
                }
            }
            _ => self.common(off, size, w),
        }
    }

    pub fn redist(&mut self, off: usize, size: usize, w: Option<u64>) -> u64 {
        let n = off / 0x20000;
        let within = off % 0x20000;
        if n >= NCPU {
            return 0;
        }
        if within < 0x10000 {
            if let Some(v) = pidr2(within) {
                return v;
            }
            let mask = if size >= 8 { u64::MAX } else { (1u64 << (size * 8)) - 1 };
            return match within {
                0x8..=0xf => {
                    let full = (n as u64) << 32 | (n as u64) << 8 | if n + 1 == NCPU { 1 << 4 } else { 0 };
                    full >> ((within - 8) * 8) & mask
                }
                0x14 => {
                    let r = self.waker as u64;
                    if let Some(v) = w {
                        let v = v as u32;
                        self.waker = (v & 2) | if v & 2 != 0 { 4 } else { 0 };
                    }
                    r
                }
                _ => 0,
            };
        } else {
            let o = within - 0x10000;
            if let Some(v) = pidr2(o) {
                return v;
            }
            if n == 0 {
                self.common(o, size, w)
            } else {
                0
            }
        }
    }

    fn pending_of(&self, id: usize) -> bool {
        (self.bits[2][id] || self.level[id]) && !self.bits[3][id]
    }

    fn run_prio(&self) -> u8 {
        self.running.last().map(|x| x.0).unwrap_or(0xff)
    }

    pub fn best(&self, any: bool) -> Option<(u8, usize, bool)> {
        let mut best: Option<(u8, usize, bool)> = None;
        let rp = self.run_prio();
        for id in 0..NINT {
            if !self.bits[1][id] || !self.pending_of(id) {
                continue;
            }
            let g1 = self.bits[0][id];
            if !any {
                if g1 && self.igrpen1 & 1 == 0 {
                    continue;
                }
                if !g1 && self.igrpen0 & 1 == 0 {
                    continue;
                }
            }
            let p = self.prio[id];
            if !any && (p as u64 >= self.pmr || p >= rp) {
                continue;
            }
            if best.map(|b| p < b.0).unwrap_or(true) {
                best = Some((p, id, g1));
            }
        }
        best
    }

    pub fn ack(&mut self, group1: bool) -> u64 {
        match self.best(false) {
            Some((p, id, g1)) if g1 == group1 => {
                self.bits[3][id] = true;
                if !self.level[id] {
                    self.bits[2][id] = false;
                }
                self.running.push((p, id));
                id as u64
            }
            _ => 1023,
        }
    }

    pub fn eoi(&mut self, id: usize) {
        if let Some(pos) = self.running.iter().rposition(|x| x.1 == id) {
            self.running.remove(pos);
        } else {
            self.running.pop();
        }
        if self.ctlr & 2 == 0 && id < NINT {
            self.bits[3][id] = false;
        }
    }

    pub fn deactivate(&mut self, id: usize) {
        if id < NINT {
            self.bits[3][id] = false;
        }
    }

    pub fn hppir(&self, group1: bool) -> u64 {
        match self.best(false) {
            Some((_, id, g1)) if g1 == group1 => id as u64,
            _ => 1023,
        }
    }

    pub fn rpr(&self) -> u64 {
        self.run_prio() as u64
    }

    pub fn sgi(&mut self, v: u64) {
        let id = ((v >> 24) & 0xf) as usize;
        if v & (1 << 40) != 0 || v & 1 != 0 {
            self.bits[2][id] = true;
        }
    }

    pub fn deliverable(&self) -> Option<bool> {
        if self.dctl & 3 == 0 && false {
            return None;
        }
        self.best(false).map(|b| !b.2)
    }

    pub fn wake(&self) -> bool {
        self.best(true).is_some()
    }
}
