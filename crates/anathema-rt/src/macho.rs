use alloc::{string::String, vec::Vec};
pub struct Seg {
    pub name: String,
    pub vm: u64,
    pub vsize: u64,
    pub off: u64,
    pub fsize: u64,
}

pub struct ChainSeg {
    pub seg_off: u64,
    pub page_size: u64,
    pub format: u16,
    pub starts: Vec<u16>,
}

pub struct Image {
    pub chains: Vec<ChainSeg>,
    pub segs: Vec<Seg>,
    pub entry: u64,
    pub chain_starts: Vec<u64>,
    pub chain_format: u32,
}

fn u32le(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

fn u64le(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}

fn name(d: &[u8], o: usize) -> String {
    let raw = &d[o..o + 16];
    let end = raw.iter().position(|&b| b == 0).unwrap_or(16);
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

pub fn parse(d: &[u8]) -> Image {
    assert_eq!(u32le(d, 0), 0xfeedfacf, "not a 64-bit mach-o");
    let ncmds = u32le(d, 16) as usize;
    let mut off = 32;
    let mut segs = Vec::new();
    let mut entry = 0;
    let mut chain_starts = Vec::new();
    let mut chain_format = 0;
    let mut chains = Vec::new();
    for _ in 0..ncmds {
        let cmd = u32le(d, off);
        let size = u32le(d, off + 4) as usize;
        if cmd == 0x19 {
            let seg = Seg {
                name: name(d, off + 8),
                vm: u64le(d, off + 24),
                vsize: u64le(d, off + 32),
                off: u64le(d, off + 40),
                fsize: u64le(d, off + 48),
            };
            let nsects = u32le(d, off + 64) as usize;
            for s in 0..nsects {
                let so = off + 72 + s * 80;
                if name(d, so) == "__chain_starts" {
                    let foff = u32le(d, so + 48) as usize;
                    chain_format = u32le(d, foff);
                    let n = u32le(d, foff + 4) as usize;
                    for i in 0..n {
                        chain_starts.push(u32le(d, foff + 8 + i * 4) as u64);
                    }
                }
            }
            segs.push(seg);
        } else if cmd == 0x8000_0034 {
            let dataoff = u32le(d, off + 8) as usize;
            let seg_count = u32le(d, dataoff + u32le(d, dataoff + 4) as usize) as usize;
            let sbase = dataoff + u32le(d, dataoff + 4) as usize;
            for i in 0..seg_count {
                let so = u32le(d, sbase + 4 + i * 4) as usize;
                if so == 0 {
                    continue;
                }
                let a = sbase + so;
                let page_size = u16::from_le_bytes(d[a + 4..a + 6].try_into().unwrap()) as u64;
                let format = u16::from_le_bytes(d[a + 6..a + 8].try_into().unwrap());
                let seg_off = u64le(d, a + 8);
                let page_count = u16::from_le_bytes(d[a + 20..a + 22].try_into().unwrap()) as usize;
                let mut starts = Vec::new();
                for k in 0..page_count {
                    starts.push(u16::from_le_bytes(d[a + 22 + k * 2..a + 24 + k * 2].try_into().unwrap()));
                }
                chains.push(ChainSeg { seg_off, page_size, format, starts });
            }
        } else if cmd == 0x5 {
            entry = u64le(d, off + 16 + 32 * 8);
        }
        off += size;
    }
    Image { chains, segs, entry, chain_starts, chain_format }
}

impl Image {
    pub fn lo(&self) -> u64 {
        self.segs.iter().filter(|s| s.vsize > 0).map(|s| s.vm).min().unwrap()
    }

    pub fn hi(&self) -> u64 {
        self.segs.iter().filter(|s| s.vsize > 0).map(|s| s.vm + s.vsize).max().unwrap()
    }

    pub fn va_of_fileoff(&self, fo: u64) -> Option<u64> {
        self.segs.iter().find(|s| s.fsize > 0 && fo >= s.off && fo < s.off + s.fsize).map(|s| s.vm + (fo - s.off))
    }
}
