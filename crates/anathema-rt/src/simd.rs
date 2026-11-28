use alloc::vec::Vec;
use anathema_xlat::run::Ctx;

type V = [u8; 16];

fn rl(q: &V, e: usize, i: usize) -> u64 {
    let mut b = [0u8; 8];
    b[..e].copy_from_slice(&q[i * e..(i + 1) * e]);
    u64::from_le_bytes(b)
}

fn wl(q: &mut V, e: usize, i: usize, v: u64) {
    q[i * e..(i + 1) * e].copy_from_slice(&v.to_le_bytes()[..e]);
}

fn mk(e: usize) -> u64 {
    if e >= 8 {
        u64::MAX
    } else {
        (1u64 << (e * 8)) - 1
    }
}

fn sx(v: u64, e: usize) -> i64 {
    let sh = 64 - e as u32 * 8;
    ((v << sh) as i64) >> sh
}

fn field(w: u32, hi: u32, lo: u32) -> u32 {
    (w >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

pub fn fallback(ctx: &mut Ctx, w: u32) -> bool {
    if w & 0x9f20_0400 == 0x0e20_0400 && pairwise3(ctx, w) {
        return true;
    }
    if w & 0x9f3e_0c00 == 0x0e20_0800 && pairwise_long(ctx, w) {
        return true;
    }
    if w >> 24 == 0xce && sha3(ctx, w) {
        return true;
    }
    if w & 0x9f20_0c00 == 0x0e20_0000 && three_diff(ctx, w) {
        return true;
    }
    if w & 0xbf20_fc00 == 0x0e20_e000 {
        let q = field(w, 30, 30) == 1;
        let size = field(w, 23, 22);
        let rm = field(w, 20, 16) as usize;
        let rn = field(w, 9, 5) as usize;
        let rd = field(w, 4, 0) as usize;
        let n = ctx.q[rn];
        let m = ctx.q[rm];
        let mut o = [0u8; 16];
        if size == 3 {
            let a = rl(&n, 8, if q { 1 } else { 0 });
            let b = rl(&m, 8, if q { 1 } else { 0 });
            let mut r: u128 = 0;
            for i in 0..64 {
                if (b >> i) & 1 == 1 {
                    r ^= (a as u128) << i;
                }
            }
            o = r.to_le_bytes();
        } else if size == 0 {
            let base = if q { 8 } else { 0 };
            for i in 0..8 {
                let a = n[base + i] as u16;
                let b = m[base + i] as u16;
                let mut r: u16 = 0;
                for k in 0..8 {
                    if (b >> k) & 1 == 1 {
                        r ^= a << k;
                    }
                }
                o[i * 2] = r as u8;
                o[i * 2 + 1] = (r >> 8) as u8;
            }
        } else {
            return false;
        }
        ctx.q[rd] = o;
        return true;
    }
    if w & 0xffe0_8400 == 0x5e00_0400 {
        let imm5 = field(w, 20, 16);
        if imm5 != 0 && imm5.trailing_zeros() <= 3 {
            let k = imm5.trailing_zeros() as usize;
            let e = 1usize << k;
            let idx = (imm5 >> (k + 1)) as usize;
            let v = rl(&ctx.q[field(w, 9, 5) as usize], e, idx);
            let mut o = [0u8; 16];
            wl(&mut o, e, 0, v);
            ctx.q[field(w, 4, 0) as usize] = o;
            return true;
        }
    }
    if w & 0xdf3e_0c00 == 0x5e20_0800 && scalar2(ctx, w) {
        return true;
    }
    if w & 0xffe0_8c00 == 0x5e00_0000 || w & 0xffff_0c00 == 0x5e28_0800 || w & 0xffff_0c00 == 0x4e28_0800 {
        return crypto(ctx, w);
    }
    if w & 0x9f00_0400 == 0x0f00_0000 {
        return by_elem(ctx, w);
    }
    if w & 0xfffffc00 == 0x5ef1b800 {
        let n = ctx.q[field(w, 9, 5) as usize];
        let v = rl(&n, 8, 0).wrapping_add(rl(&n, 8, 1));
        let mut out = [0u8; 16];
        wl(&mut out, 8, 0, v);
        ctx.q[field(w, 4, 0) as usize] = out;
        return true;
    }
    if w & 0xbfbf_fc00 == 0x0ea0_f800 || w & 0xbfbf_fc00 == 0x2ea0_f800 {
        let q = field(w, 30, 30) == 1;
        let neg = field(w, 29, 29) == 1;
        let dbl = field(w, 22, 22) == 1;
        let rn = field(w, 9, 5) as usize;
        let rd = field(w, 4, 0) as usize;
        if dbl && !q {
            return false;
        }
        let e = if dbl { 8 } else { 4 };
        let sign = 1u64 << (e * 8 - 1);
        let n = ctx.q[rn];
        let mut out = [0u8; 16];
        for i in 0..(if q { 16 } else { 8 }) / e {
            let a = rl(&n, e, i);
            wl(&mut out, e, i, if neg { a ^ sign } else { a & !sign });
        }
        ctx.q[rd] = out;
        return true;
    }
    if w & 0x9f3e_0c00 == 0x0e30_0800 {
        return across(ctx, w);
    }
    if w & 0x9f20_0400 == 0x0e20_0400 {
        return three_same(ctx, w);
    }
    if w & 0x9f3e_0c00 == 0x0e20_0800 {
        return two_reg(ctx, w);
    }
    if w & 0xbfe0_8400 == 0x2e00_0000 {
        return ext(ctx, w);
    }
    if w & 0xbf20_8c00 == 0x0e00_0800 {
        return perm(ctx, w);
    }
    if w & 0xbfe0_9c00 == 0x0e00_0000 {
        return tbl(ctx, w);
    }
    if w & 0x9fe0_8400 == 0x0e00_0400 {
        return vcopy(ctx, w);
    }
    if w & 0x9ff8_0400 == 0x0f00_0400 {
        return mod_imm(ctx, w);
    }
    if w & 0x9f80_0400 == 0x0f00_0400 {
        return shift_imm(ctx, w);
    }
    false
}

fn mod_imm(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let op = field(w, 29, 29);
    let cmode = field(w, 15, 12);
    let imm8 = ((field(w, 18, 16) << 5) | field(w, 9, 5)) as u64;
    let rd = field(w, 4, 0) as usize;
    let mut pat = [0u8; 16];
    if field(w, 11, 11) == 1 {
        if cmode != 15 {
            return false;
        }
        let a = (imm8 >> 7) & 1;
        let b = (imm8 >> 6) & 1;
        let cdefgh = imm8 & 0x3f;
        let v = (a << 15) | ((b ^ 1) << 14) | (b << 13) | (b << 12) | (cdefgh << 6);
        let total = if q { 16 } else { 8 };
        let mut out = [0u8; 16];
        for i in 0..total / 2 {
            wl(&mut out, 2, i, v);
        }
        ctx.q[rd] = out;
        return true;
    }
    let (e, val, kind): (usize, u64, u8) = match cmode {
        0..=7 => (4, imm8 << (8 * ((cmode >> 1) & 3)), if cmode & 1 == 0 { 0 } else { 1 }),
        8..=11 => (2, imm8 << (8 * ((cmode >> 1) & 1)), if cmode & 1 == 0 { 0 } else { 1 }),
        12 | 13 => {
            let sh = if cmode == 12 { 8 } else { 16 };
            (4, (imm8 << sh) | ((1u64 << sh) - 1), 0)
        }
        14 if op == 0 => (1, imm8, 0),
        14 => {
            let mut v = 0u64;
            for b in 0..8 {
                if (imm8 >> b) & 1 == 1 {
                    v |= 0xffu64 << (b * 8);
                }
            }
            (8, v, 3)
        }
        _ => {
            if op == 0 {
                let a = (imm8 >> 7) & 1;
                let b = (imm8 >> 6) & 1;
                let cdefgh = imm8 & 0x3f;
                let v = (a << 31) | ((b ^ 1) << 30) | (if b == 1 { 0x1f } else { 0 } << 25) | (cdefgh << 19);
                (4, v, 3)
            } else {
                if !q {
                    return false;
                }
                let a = (imm8 >> 7) & 1;
                let b = (imm8 >> 6) & 1;
                let cdefgh = imm8 & 0x3f;
                let v = (a << 63) | ((b ^ 1) << 62) | (if b == 1 { 0xffu64 } else { 0 } << 54) | (cdefgh << 48);
                (8, v, 3)
            }
        }
    };
    let total = if q { 16 } else { 8 };
    for i in 0..16 / e {
        wl(&mut pat, e, i, val & mk(e));
    }
    let inv = op == 1 && cmode < 14 && kind == 0;
    let mut out = [0u8; 16];
    for i in 0..total {
        let old = ctx.q[rd][i];
        out[i] = match (kind, op) {
            (1, 0) => old | pat[i],
            (1, 1) => old & !pat[i],
            _ => if inv { !pat[i] } else { pat[i] },
        };
    }
    ctx.q[rd] = out;
    true
}

fn across(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30);
    let u = field(w, 29, 29);
    let size = field(w, 23, 22) as usize;
    let opcode = field(w, 16, 12);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let e = 1usize << size;
    if e == 8 {
        return false;
    }
    let count = if q == 1 { 16 } else { 8 } / e;
    let m = mk(e);
    let src: Vec<u64> = (0..count).map(|i| rl(&ctx.q[rn], e, i)).collect();
    let (res, osz): (u64, usize) = match (opcode, u) {
        (0x03, 0) => (src.iter().fold(0i64, |a, &v| a.wrapping_add(sx(v, e))) as u64, e * 2),
        (0x03, 1) => (src.iter().fold(0u64, |a, &v| a.wrapping_add(v & m)), e * 2),
        (0x1b, _) => (src.iter().fold(0u64, |a, &v| a.wrapping_add(v)) & m, e),
        (0x0a, 0) => (src.iter().map(|&v| sx(v, e)).max().unwrap() as u64 & m, e),
        (0x0a, 1) => (src.iter().map(|&v| v & m).max().unwrap(), e),
        (0x1a, 0) => (src.iter().map(|&v| sx(v, e)).min().unwrap() as u64 & m, e),
        (0x1a, 1) => (src.iter().map(|&v| v & m).min().unwrap(), e),
        _ => return false,
    };
    let mut out = [0u8; 16];
    out[..osz.min(8)].copy_from_slice(&res.to_le_bytes()[..osz.min(8)]);
    ctx.q[rd] = out;
    true
}

fn three_same(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let u = field(w, 29, 29);
    let size = field(w, 23, 22) as usize;
    let rm = field(w, 20, 16) as usize;
    let opcode = field(w, 15, 11);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let n = ctx.q[rn];
    let mv = ctx.q[rm];
    let d = ctx.q[rd];
    let total = if q { 16 } else { 8 };
    let mut out = [0u8; 16];
    if opcode == 0x03 {
        for i in 0..total {
            let (a, b, c) = (n[i], mv[i], d[i]);
            out[i] = match (u, size) {
                (0, 0) => a & b,
                (0, 1) => a & !b,
                (0, 2) => a | b,
                (0, _) => a | !b,
                (_, 0) => a ^ b,
                (_, 1) => (c & a) | (!c & b),
                (_, 2) => (c & !b) | (a & b),
                (_, _) => (c & b) | (a & !b),
            };
        }
        ctx.q[rd] = out;
        return true;
    }
    let e = 1usize << size;
    if e == 8 && !matches!(opcode, 0x06 | 0x07 | 0x08 | 0x10 | 0x11 | 0x0c | 0x0d | 0x0e) {
        return false;
    }
    let count = total / e;
    let m = mk(e);
    let pair = matches!(opcode, 0x14 | 0x15 | 0x17);
    let src: Vec<u64> = if pair {
        let mut s: Vec<u64> = (0..count).map(|i| rl(&n, e, i)).collect();
        s.extend((0..count).map(|i| rl(&mv, e, i)));
        s
    } else {
        Vec::new()
    };
    for i in 0..count {
        let a = rl(&n, e, i);
        let b = rl(&mv, e, i);
        let c = rl(&d, e, i);
        let (sa, sb) = (sx(a, e), sx(b, e));
        let r: u64 = match (opcode, u) {
            (0x00, 0) => (((sa as i128 + sb as i128) >> 1) as u64) & m,
            (0x00, _) => (((a as u128 + b as u128) >> 1) as u64) & m,
            (0x04, 0) => (((sa as i128 - sb as i128) >> 1) as u64) & m,
            (0x04, _) => (((a as i128 - b as i128) >> 1) as u64) & m,
            (0x06, 0) => if sa > sb { m } else { 0 },
            (0x06, _) => if a > b { m } else { 0 },
            (0x07, 0) => if sa >= sb { m } else { 0 },
            (0x07, _) => if a >= b { m } else { 0 },
            (0x08, _) => {
                let sh = sb as i8 as i32;
                if sh >= 0 {
                    if sh as usize >= e * 8 { 0 } else { (a << sh) & m }
                } else if u == 0 {
                    ((sa >> (-sh).min(63)) as u64) & m
                } else if (-sh) as usize >= e * 8 {
                    0
                } else {
                    (a >> (-sh)) & m
                }
            }
            (0x0c, 0) => (sa.max(sb) as u64) & m,
            (0x0c, _) => a.max(b),
            (0x0d, 0) => (sa.min(sb) as u64) & m,
            (0x0d, _) => a.min(b),
            (0x0e, 0) => ((sa - sb).unsigned_abs()) & m,
            (0x0e, _) => (if a > b { a - b } else { b - a }) & m,
            (0x0f, 0) => c.wrapping_add((sa - sb).unsigned_abs()) & m,
            (0x0f, _) => c.wrapping_add(if a > b { a - b } else { b - a }) & m,
            (0x10, 0) => a.wrapping_add(b) & m,
            (0x10, _) => a.wrapping_sub(b) & m,
            (0x11, 0) => if a & b != 0 { m } else { 0 },
            (0x11, _) => if a == b { m } else { 0 },
            (0x12, 0) => c.wrapping_add(a.wrapping_mul(b)) & m,
            (0x12, _) => c.wrapping_sub(a.wrapping_mul(b)) & m,
            (0x13, 0) => a.wrapping_mul(b) & m,
            (0x14, 0) => pm(&src, i, count, e, true, true),
            (0x14, _) => pm(&src, i, count, e, true, false),
            (0x15, 0) => pm(&src, i, count, e, false, true),
            (0x15, _) => pm(&src, i, count, e, false, false),
            (0x17, 0) => {
                let (x, y) = src_pair(&src, i, count);
                x.wrapping_add(y) & m
            }
            _ => return false,
        };
        wl(&mut out, e, i, r);
    }
    ctx.q[rd] = out;
    true
}

fn src_pair(src: &[u64], i: usize, count: usize) -> (u64, u64) {
    let _ = count;
    (src[2 * i], src[2 * i + 1])
}

fn pm(src: &[u64], i: usize, count: usize, e: usize, max: bool, signed: bool) -> u64 {
    let _ = count;
    let (x, y) = (src[2 * i], src[2 * i + 1]);
    let m = mk(e);
    if signed {
        let (a, b) = (sx(x, e), sx(y, e));
        (if max { a.max(b) } else { a.min(b) }) as u64 & m
    } else if max {
        x.max(y)
    } else {
        x.min(y)
    }
}

fn two_reg(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let u = field(w, 29, 29);
    let size = field(w, 23, 22) as usize;
    let opcode = field(w, 16, 12);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let n = ctx.q[rn];
    let total = if q { 16 } else { 8 };
    let e = 1usize << size;
    let mut out = [0u8; 16];
    match (opcode, u) {
        (0x05, 0) if size == 0 => {
            for i in 0..total {
                out[i] = n[i].count_ones() as u8;
            }
        }
        (0x05, 1) if size == 0 => {
            for i in 0..total {
                out[i] = !n[i];
            }
        }
        (0x05, 1) if size == 1 => {
            for i in 0..total {
                out[i] = n[i].reverse_bits();
            }
        }
        (0x00, _) | (0x01, 0) => {
            let container = if opcode == 0x01 { 2 } else if u == 0 { 8 } else { 4 };
            if e >= container {
                return false;
            }
            let per = container / e;
            for c in 0..total / container {
                for j in 0..per {
                    let src = rl(&n, e, c * per + j);
                    wl(&mut out, e, c * per + (per - 1 - j), src);
                }
            }
        }
        (0x04, 1) if e < 8 => {
            let m = mk(e);
            for i in 0..total / e {
                let a = rl(&n, e, i) & m;
                let bits = e as u32 * 8;
                let lz = if a == 0 { bits } else { a.leading_zeros() - (64 - bits) };
                wl(&mut out, e, i, lz as u64);
            }
        }
        (0x08, _) | (0x09, _) | (0x0a, 0) if e <= 8 => {
            let m = mk(e);
            for i in 0..total / e {
                let a = sx(rl(&n, e, i), e);
                let t = match (opcode, u) {
                    (0x08, 0) => a > 0,
                    (0x08, _) => a >= 0,
                    (0x09, 0) => a == 0,
                    (0x09, _) => a <= 0,
                    _ => a < 0,
                };
                wl(&mut out, e, i, if t { m } else { 0 });
            }
        }
        (0x0b, _) if e <= 8 => {
            let m = mk(e);
            for i in 0..total / e {
                let a = sx(rl(&n, e, i), e);
                let r = if u == 0 { a.wrapping_abs() } else { a.wrapping_neg() };
                wl(&mut out, e, i, r as u64 & m);
            }
        }
        (0x12, 0) if e < 8 => {
            let wide = e * 2;
            let half = 8 / e;
            let mut base = if q { ctx.q[rd] } else { [0u8; 16] };
            let lanes = 8 / e;
            for i in 0..lanes {
                let v = rl(&n, wide, i) & mk(e);
                if q {
                    wl(&mut base, e, half + i, v);
                } else {
                    wl(&mut base, e, i, v);
                }
            }
            ctx.q[rd] = base;
            return true;
        }
        _ => return false,
    }
    ctx.q[rd] = out;
    true
}

fn ext(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let rm = field(w, 20, 16) as usize;
    let imm = field(w, 14, 11) as usize;
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let total = if q { 16 } else { 8 };
    let mut cat = [0u8; 32];
    cat[..total].copy_from_slice(&ctx.q[rn][..total]);
    cat[total..total * 2].copy_from_slice(&ctx.q[rm][..total]);
    let mut out = [0u8; 16];
    for i in 0..total {
        out[i] = cat[imm + i];
    }
    ctx.q[rd] = out;
    true
}

fn perm(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let size = field(w, 23, 22) as usize;
    let rm = field(w, 20, 16) as usize;
    let opc = field(w, 14, 12);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let e = 1usize << size;
    let total = if q { 16 } else { 8 };
    let count = total / e;
    if e == 8 && !q {
        return false;
    }
    let n = ctx.q[rn];
    let m = ctx.q[rm];
    let mut out = [0u8; 16];
    for i in 0..count {
        let v = match opc {
            1 => {
                let j = 2 * i;
                if j < count { rl(&n, e, j) } else { rl(&m, e, j - count) }
            }
            5 => {
                let j = 2 * i + 1;
                if j < count { rl(&n, e, j) } else { rl(&m, e, j - count) }
            }
            2 => if i % 2 == 0 { rl(&n, e, i) } else { rl(&m, e, i - 1) },
            6 => if i % 2 == 0 { rl(&n, e, i + 1) } else { rl(&m, e, i) },
            3 => {
                let j = i / 2;
                if i % 2 == 0 { rl(&n, e, j) } else { rl(&m, e, j) }
            }
            7 => {
                let j = count / 2 + i / 2;
                if i % 2 == 0 { rl(&n, e, j) } else { rl(&m, e, j) }
            }
            _ => return false,
        };
        wl(&mut out, e, i, v);
    }
    ctx.q[rd] = out;
    true
}

fn tbl(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let rm = field(w, 20, 16) as usize;
    let len = field(w, 14, 13) as usize + 1;
    let tbx = field(w, 12, 12) == 1;
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let total = if q { 16 } else { 8 };
    let idx = ctx.q[rm];
    let old = ctx.q[rd];
    let mut table = Vec::new();
    for k in 0..len {
        table.extend_from_slice(&ctx.q[(rn + k) % 32]);
    }
    let mut out = [0u8; 16];
    for i in 0..total {
        let x = idx[i] as usize;
        out[i] = if x < table.len() { table[x] } else if tbx { old[i] } else { 0 };
    }
    ctx.q[rd] = out;
    true
}

fn shift_imm(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let u = field(w, 29, 29);
    let immh = field(w, 22, 19) as usize;
    let immb = field(w, 18, 16) as usize;
    let opcode = field(w, 15, 11);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    if immh == 0 {
        return false;
    }
    let k = 31 - (immh as u32).leading_zeros();
    let e = 1usize << k;
    let ebits = e * 8;
    let immhb = (immh << 3) | immb;
    let n = ctx.q[rn];
    let total = if q { 16 } else { 8 };
    let m = mk(e);
    let mut out = [0u8; 16];
    match opcode {
        0x00 | 0x02 => {
            let sh = 2 * ebits - immhb;
            for i in 0..total / e {
                let a = rl(&n, e, i);
                let r = if u == 0 {
                    ((sx(a, e) >> sh.min(63)) as u64) & m
                } else if sh >= 64 {
                    0
                } else {
                    (a >> sh) & m
                };
                let base = if opcode == 0x02 { rl(&ctx.q[rd], e, i) } else { 0 };
                wl(&mut out, e, i, base.wrapping_add(r) & m);
            }
        }
        0x0a => {
            let sh = immhb - ebits;
            for i in 0..total / e {
                let a = rl(&n, e, i);
                let r = if u == 0 {
                    (a << sh) & m
                } else {
                    let keep = !(m << sh) & m;
                    (rl(&ctx.q[rd], e, i) & keep) | ((a << sh) & m)
                };
                wl(&mut out, e, i, r);
            }
        }
        0x14 if e < 8 => {
            let sh = immhb - ebits;
            let wide = e * 2;
            let half = if q { 8 / e } else { 0 };
            for i in 0..8 / e {
                let a = rl(&n, e, half + i);
                let v = if u == 0 { (sx(a, e) << sh) as u64 } else { a << sh };
                wl(&mut out, wide, i, v & mk(wide));
            }
        }
        0x10 if e < 8 && u == 0 => {
            let sh = 2 * ebits - immhb;
            let narrow = e;
            let src = e * 2;
            let half = if q { 8 / narrow } else { 0 };
            let mut base = if q { ctx.q[rd] } else { [0u8; 16] };
            for i in 0..8 / narrow {
                let a = rl(&n, src, i);
                let v = (a >> sh) & mk(narrow);
                wl(&mut base, narrow, half + i, v);
            }
            ctx.q[rd] = base;
            return true;
        }
        _ => return false,
    }
    ctx.q[rd] = out;
    true
}

fn vcopy(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let op = field(w, 29, 29);
    let imm5 = field(w, 20, 16);
    let imm4 = field(w, 14, 11) as usize;
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    if imm5 == 0 {
        return false;
    }
    let k = imm5.trailing_zeros() as usize;
    if k > 3 {
        return false;
    }
    let e = 1usize << k;
    let idx = (imm5 >> (k + 1)) as usize;
    let gp = |ctx: &Ctx, r: usize| if r == 31 { 0 } else { ctx.x[r] };
    if op == 1 {
        let sidx = imm4 >> k;
        let v = rl(&ctx.q[rn], e, sidx);
        let mut d = ctx.q[rd];
        wl(&mut d, e, idx, v);
        ctx.q[rd] = d;
        return true;
    }
    match imm4 {
        0 => {
            let v = rl(&ctx.q[rn], e, idx);
            let total = if q { 16 } else { 8 };
            let mut out = [0u8; 16];
            for i in 0..total / e {
                wl(&mut out, e, i, v);
            }
            ctx.q[rd] = out;
        }
        1 => {
            let v = gp(ctx, rn) & mk(e);
            let total = if q { 16 } else { 8 };
            let mut out = [0u8; 16];
            for i in 0..total / e {
                wl(&mut out, e, i, v);
            }
            ctx.q[rd] = out;
        }
        3 => {
            let v = gp(ctx, rn) & mk(e);
            let mut d = ctx.q[rd];
            wl(&mut d, e, idx, v);
            ctx.q[rd] = d;
        }
        5 | 7 => {
            let v = rl(&ctx.q[rn], e, idx);
            let v = if imm4 == 5 { if q { sx(v, e) as u64 } else { (sx(v, e) as u64) & 0xffff_ffff } } else { v };
            if rd != 31 {
                ctx.x[rd] = v;
            }
        }
        _ => return false,
    }
    true
}

fn by_elem(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let u = field(w, 29, 29) == 1;
    let size = field(w, 23, 22) as usize;
    let l = field(w, 21, 21) as usize;
    let m = field(w, 20, 20) as usize;
    let rmlo = field(w, 19, 16) as usize;
    let opcode = field(w, 15, 12);
    let h = field(w, 11, 11) as usize;
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let (e, idx, rm) = match size {
        1 => (2usize, (h << 2) | (l << 1) | m, rmlo),
        2 => (4usize, (h << 1) | l, (m << 4) | rmlo),
        _ => return false,
    };
    let n = ctx.q[rn];
    let s = rl(&ctx.q[rm], e, idx);
    let mm = mk(e);
    let mut out = [0u8; 16];
    match opcode {
        0x0 | 0x4 | 0x8 if (opcode == 0x8 && !u) || (opcode != 0x8 && u) => {
            let total = if q { 16 } else { 8 };
            for i in 0..total / e {
                let p = rl(&n, e, i).wrapping_mul(s) & mm;
                let old = rl(&ctx.q[rd], e, i);
                let r = match opcode {
                    0x0 => old.wrapping_add(p),
                    0x4 => old.wrapping_sub(p),
                    _ => p,
                } & mm;
                wl(&mut out, e, i, r);
            }
        }
        0x2 | 0x6 | 0xa => {
            let lanes = 8 / e;
            let base = if q { lanes } else { 0 };
            let we = e * 2;
            let wm = mk(we);
            for i in 0..lanes {
                let a = rl(&n, e, base + i);
                let p = if u {
                    a.wrapping_mul(s)
                } else {
                    (sx(a, e).wrapping_mul(sx(s, e))) as u64
                } & wm;
                let old = rl(&ctx.q[rd], we, i);
                let r = match opcode {
                    0x2 => old.wrapping_add(p),
                    0x6 => old.wrapping_sub(p),
                    _ => p,
                } & wm;
                wl(&mut out, we, i, r);
            }
        }
        _ => return false,
    }
    ctx.q[rd] = out;
    true
}

fn lanes(v: &V) -> [u32; 4] {
    [rl(v, 4, 0) as u32, rl(v, 4, 1) as u32, rl(v, 4, 2) as u32, rl(v, 4, 3) as u32]
}

fn unl(l: [u32; 4]) -> V {
    let mut o = [0u8; 16];
    for i in 0..4 {
        wl(&mut o, 4, i, l[i] as u64);
    }
    o
}

fn sboxes() -> &'static ([u8; 256], [u8; 256]) {
    static mut T: Option<([u8; 256], [u8; 256])> = None;
    let cell = unsafe { &mut *core::ptr::addr_of_mut!(T) };
    if cell.is_some() {
        return cell.as_ref().unwrap();
    }
    let built = (|| {
        let mut sb = [0u8; 256];
        let mut inv = [0u8; 256];
        let mut p: u8 = 1;
        let mut q: u8 = 1;
        loop {
            p = p ^ (p << 1) ^ if p & 0x80 != 0 { 0x1b } else { 0 };
            q ^= q << 1;
            q ^= q << 2;
            q ^= q << 4;
            if q & 0x80 != 0 {
                q ^= 0x09;
            }
            let x = q ^ q.rotate_left(1) ^ q.rotate_left(2) ^ q.rotate_left(3) ^ q.rotate_left(4);
            sb[p as usize] = x ^ 0x63;
            if p == 1 {
                break;
            }
        }
        sb[0] = 0x63;
        for i in 0..256 {
            inv[sb[i] as usize] = i as u8;
        }
        (sb, inv)
    })();
    *cell = Some(built);
    cell.as_ref().unwrap()
}

fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut r = 0u8;
    while b != 0 {
        if b & 1 != 0 {
            r ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    r
}

fn crypto(ctx: &mut Ctx, w: u32) -> bool {
    let rm = field(w, 20, 16) as usize;
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let d = ctx.q[rd];
    let n = ctx.q[rn];
    let m = ctx.q[rm];
    let ch = |x: u32, y: u32, z: u32| (x & y) | (!x & z);
    let maj = |x: u32, y: u32, z: u32| (x & y) | (x & z) | (y & z);
    if w & 0xffe0_8c00 == 0x5e00_0000 {
        let op = field(w, 14, 12);
        match op {
            0 | 1 | 2 => {
                let mut x = lanes(&d);
                let mut y = rl(&n, 4, 0) as u32;
                let wv = lanes(&m);
                for e in 0..4 {
                    let t = match op {
                        0 => ch(x[1], x[2], x[3]),
                        1 => x[1] ^ x[2] ^ x[3],
                        _ => maj(x[1], x[2], x[3]),
                    };
                    y = y.wrapping_add(x[0].rotate_left(5)).wrapping_add(t).wrapping_add(wv[e]);
                    x[1] = x[1].rotate_left(30);
                    let ny = x[3];
                    x = [y, x[0], x[1], x[2]];
                    y = ny;
                }
                ctx.q[rd] = unl(x);
            }
            3 => {
                let a = lanes(&d);
                let b = lanes(&n);
                let c = lanes(&m);
                let t = [a[2], a[3], b[0], b[1]];
                ctx.q[rd] = unl([t[0] ^ a[0] ^ c[0], t[1] ^ a[1] ^ c[1], t[2] ^ a[2] ^ c[2], t[3] ^ a[3] ^ c[3]]);
            }
            4 | 5 => {
                let (mut x, mut y) = if op == 4 { (lanes(&d), lanes(&n)) } else { (lanes(&n), lanes(&d)) };
                let wv = lanes(&m);
                for e in 0..4 {
                    let chs = ch(y[0], y[1], y[2]);
                    let mj = maj(x[0], x[1], x[2]);
                    let s1 = y[0].rotate_right(6) ^ y[0].rotate_right(11) ^ y[0].rotate_right(25);
                    let s0 = x[0].rotate_right(2) ^ x[0].rotate_right(13) ^ x[0].rotate_right(22);
                    let t = y[3].wrapping_add(s1).wrapping_add(chs).wrapping_add(wv[e]);
                    x[3] = t.wrapping_add(x[3]);
                    y[3] = t.wrapping_add(s0).wrapping_add(mj);
                    let nx = [y[3], x[0], x[1], x[2]];
                    let ny = [x[3], y[0], y[1], y[2]];
                    x = nx;
                    y = ny;
                }
                ctx.q[rd] = unl(if op == 4 { x } else { y });
            }
            6 => {
                let x = lanes(&d);
                let y = lanes(&n);
                let z = lanes(&m);
                let t0 = [y[1], y[2], y[3], z[0]];
                let mut res = [0u32; 4];
                let t1 = [z[2], z[3]];
                for e in 0..2 {
                    let el = t1[e];
                    let el = el.rotate_right(17) ^ el.rotate_right(19) ^ (el >> 10);
                    res[e] = el.wrapping_add(x[e]).wrapping_add(t0[e]);
                }
                for e in 0..2 {
                    let el = res[e];
                    let el = el.rotate_right(17) ^ el.rotate_right(19) ^ (el >> 10);
                    res[e + 2] = el.wrapping_add(x[e + 2]).wrapping_add(t0[e + 2]);
                }
                ctx.q[rd] = unl(res);
            }
            _ => return false,
        }
        return true;
    }
    if w & 0xffff_0c00 == 0x5e28_0800 {
        match field(w, 15, 12) {
            0 => {
                let v = rl(&n, 4, 0) as u32;
                let mut o = [0u8; 16];
                wl(&mut o, 4, 0, v.rotate_left(30) as u64);
                ctx.q[rd] = o;
            }
            1 => {
                let x = lanes(&d);
                let y = lanes(&n);
                let t = [x[0] ^ y[1], x[1] ^ y[2], x[2] ^ y[3], x[3]];
                let w0 = t[0].rotate_left(1);
                let w1 = t[1].rotate_left(1);
                let w2 = t[2].rotate_left(1);
                let w3 = t[3].rotate_left(1) ^ t[0].rotate_left(2);
                ctx.q[rd] = unl([w0, w1, w2, w3]);
            }
            2 => {
                let x = lanes(&d);
                let y = lanes(&n);
                let t = [x[1], x[2], x[3], y[0]];
                let mut r = [0u32; 4];
                for e in 0..4 {
                    let el = t[e];
                    r[e] = (el.rotate_right(7) ^ el.rotate_right(18) ^ (el >> 3)).wrapping_add(x[e]);
                }
                ctx.q[rd] = unl(r);
            }
            _ => return false,
        }
        return true;
    }
    let (sb, inv) = sboxes();
    match field(w, 15, 12) {
        4 | 5 => {
            let dec = field(w, 12, 12) == 1;
            let mut st = [0u8; 16];
            for i in 0..16 {
                st[i] = d[i] ^ n[i];
            }
            let sr: [usize; 16] = if dec { [0, 13, 10, 7, 4, 1, 14, 11, 8, 5, 2, 15, 12, 9, 6, 3] } else { [0, 5, 10, 15, 4, 9, 14, 3, 8, 13, 2, 7, 12, 1, 6, 11] };
            let mut o = [0u8; 16];
            for i in 0..16 {
                let b = st[sr[i]];
                o[i] = if dec { inv[b as usize] } else { sb[b as usize] };
            }
            ctx.q[rd] = o;
            true
        }
        6 | 7 => {
            let dec = field(w, 12, 12) == 1;
            let mut o = [0u8; 16];
            for c in 0..4 {
                let a = [n[c * 4], n[c * 4 + 1], n[c * 4 + 2], n[c * 4 + 3]];
                let m4: [[u8; 4]; 4] = if dec { [[14, 11, 13, 9], [9, 14, 11, 13], [13, 9, 14, 11], [11, 13, 9, 14]] } else { [[2, 3, 1, 1], [1, 2, 3, 1], [1, 1, 2, 3], [3, 1, 1, 2]] };
                for r in 0..4 {
                    o[c * 4 + r] = gmul(a[0], m4[r][0]) ^ gmul(a[1], m4[r][1]) ^ gmul(a[2], m4[r][2]) ^ gmul(a[3], m4[r][3]);
                }
            }
            ctx.q[rd] = o;
            true
        }
        _ => false,
    }
}

fn scalar2(ctx: &mut Ctx, w: u32) -> bool {
    let u = field(w, 29, 29) == 1;
    let b23 = field(w, 23, 23);
    let dbl = field(w, 22, 22) == 1;
    let opcode = field(w, 16, 12);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let n = ctx.q[rn];
    let mut o = [0u8; 16];
    match (opcode, b23) {
        (0x1d, 0) => {
            if dbl {
                let v = rl(&n, 8, 0);
                let f = if u { v as f64 } else { v as i64 as f64 };
                wl(&mut o, 8, 0, f.to_bits());
            } else {
                let v = rl(&n, 4, 0) as u32;
                let f = if u { v as f32 } else { v as i32 as f32 };
                wl(&mut o, 4, 0, f.to_bits() as u64);
            }
        }
        (0x1b, 1) => {
            if dbl {
                let f = f64::from_bits(rl(&n, 8, 0));
                let v = if u { f as u64 } else { f as i64 as u64 };
                wl(&mut o, 8, 0, v);
            } else {
                let f = f32::from_bits(rl(&n, 4, 0) as u32);
                let v = if u { f as u32 } else { f as i32 as u32 };
                wl(&mut o, 4, 0, v as u64);
            }
        }
        _ => return false,
    }
    ctx.q[rd] = o;
    true
}

fn three_diff(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let u = field(w, 29, 29) == 1;
    let size = field(w, 23, 22) as usize;
    let rm = field(w, 20, 16) as usize;
    let opcode = field(w, 15, 12);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    if size == 3 {
        return false;
    }
    let e = 1usize << size;
    let we = e * 2;
    let n = ctx.q[rn];
    let m = ctx.q[rm];
    let d = ctx.q[rd];
    let lanes = 8 / e;
    let base = if q { lanes } else { 0 };
    let wm = mk(we);
    let ext = |v: u64, sz: usize| -> i128 {
        if u {
            v as i128
        } else {
            sx(v, sz) as i128
        }
    };
    let mut out = [0u8; 16];
    match opcode {
        0 | 1 | 2 | 3 | 5 | 7 | 8 | 10 | 12 => {
            for i in 0..lanes {
                let a = if opcode & 1 == 1 && opcode <= 3 { ext(rl(&n, we, i), we) } else { ext(rl(&n, e, base + i), e) };
                let b = ext(rl(&m, e, base + i), e);
                let old = rl(&d, we, i) as i128;
                let r: i128 = match opcode {
                    0 | 1 => a + b,
                    2 | 3 => a - b,
                    5 => old + (a - b).abs(),
                    7 => (a - b).abs(),
                    8 => old + a * b,
                    10 => old - a * b,
                    _ => a * b,
                };
                wl(&mut out, we, i, (r as u64) & wm);
            }
            ctx.q[rd] = out;
            true
        }
        4 | 6 => {
            let round: i128 = if u { 1i128 << (we * 8 / 2 - 1) } else { 0 };
            let mut o = if q { d } else { [0u8; 16] };
            let half = if q { lanes } else { 0 };
            for i in 0..lanes {
                let a = rl(&n, we, i) as i128;
                let b = rl(&m, we, i) as i128;
                let r = if opcode == 4 { a + b + round } else { a - b + round };
                let r = ((r as u64) & wm) >> (we * 8 / 2);
                wl(&mut o, e, half + i, r);
            }
            ctx.q[rd] = o;
            true
        }
        _ => false,
    }
}

fn sha3(ctx: &mut Ctx, w: u32) -> bool {
    let rm = field(w, 20, 16) as usize;
    let ra = field(w, 14, 10) as usize;
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let n = ctx.q[rn];
    let m = ctx.q[rm];
    let d = ctx.q[rd];
    let a = ctx.q[ra];
    let lo = |v: &V| rl(v, 8, 0);
    let hi = |v: &V| rl(v, 8, 1);
    let mk2 = |l: u64, h: u64| -> V {
        let mut o = [0u8; 16];
        wl(&mut o, 8, 0, l);
        wl(&mut o, 8, 1, h);
        o
    };
    let mut o = [0u8; 16];
    if w & 0xffe0_0000 == 0xce00_0000 {
        for i in 0..16 {
            o[i] = n[i] ^ m[i] ^ a[i];
        }
    } else if w & 0xffe0_0000 == 0xce20_0000 {
        for i in 0..16 {
            o[i] = n[i] ^ (m[i] & !a[i]);
        }
    } else if w & 0xffe0_fc00 == 0xce60_8c00 {
        o = mk2(lo(&n) ^ lo(&m).rotate_left(1), hi(&n) ^ hi(&m).rotate_left(1));
    } else if w & 0xffe0_0000 == 0xce80_0000 {
        let imm6 = field(w, 15, 10);
        o = mk2((lo(&n) ^ lo(&m)).rotate_right(imm6), (hi(&n) ^ hi(&m)).rotate_right(imm6));
    } else if w & 0xffe0_fc00 == 0xce60_8000 {
        let (x, y, wv) = (n, m, d);
        let ms1 = hi(&y).rotate_right(14) ^ hi(&y).rotate_right(18) ^ hi(&y).rotate_right(41);
        let mut vh = (hi(&y) & lo(&x)) ^ (!hi(&y) & hi(&x));
        vh = vh.wrapping_add(ms1).wrapping_add(hi(&wv));
        let tmp = vh.wrapping_add(lo(&y));
        let ms1b = tmp.rotate_right(14) ^ tmp.rotate_right(18) ^ tmp.rotate_right(41);
        let mut vl = (tmp & hi(&y)) ^ (!tmp & lo(&x));
        vl = vl.wrapping_add(ms1b).wrapping_add(lo(&wv));
        o = mk2(vl, vh);
    } else if w & 0xffe0_fc00 == 0xce60_8400 {
        let (x, y, wv) = (n, m, d);
        let ns0 = lo(&y).rotate_right(28) ^ lo(&y).rotate_right(34) ^ lo(&y).rotate_right(39);
        let mut vh = (lo(&x) & hi(&y)) ^ (lo(&x) & lo(&y)) ^ (hi(&y) & lo(&y));
        vh = vh.wrapping_add(ns0).wrapping_add(hi(&wv));
        let ns0b = vh.rotate_right(28) ^ vh.rotate_right(34) ^ vh.rotate_right(39);
        let mut vl = (vh & lo(&y)) ^ (vh & hi(&y)) ^ (lo(&y) & hi(&y));
        vl = vl.wrapping_add(ns0b).wrapping_add(lo(&wv));
        o = mk2(vl, vh);
    } else if w & 0xffe0_fc00 == 0xce60_8800 {
        let (x, y, wv) = (n, m, d);
        let s1h = hi(&x).rotate_right(19) ^ hi(&x).rotate_right(61) ^ (hi(&x) >> 6);
        let s1l = lo(&x).rotate_right(19) ^ lo(&x).rotate_right(61) ^ (lo(&x) >> 6);
        o = mk2(lo(&wv).wrapping_add(s1l).wrapping_add(lo(&y)), hi(&wv).wrapping_add(s1h).wrapping_add(hi(&y)));
    } else if w & 0xffff_fc00 == 0xcec0_8000 {
        let (x, wv) = (n, d);
        let s0l = hi(&wv).rotate_right(1) ^ hi(&wv).rotate_right(8) ^ (hi(&wv) >> 7);
        let s0h = lo(&x).rotate_right(1) ^ lo(&x).rotate_right(8) ^ (lo(&x) >> 7);
        o = mk2(lo(&wv).wrapping_add(s0l), hi(&wv).wrapping_add(s0h));
    } else {
        return false;
    }
    ctx.q[rd] = o;
    true
}

fn pairwise_long(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let u = field(w, 29, 29) == 1;
    let size = field(w, 23, 22) as usize;
    let opcode = field(w, 16, 12);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    if (opcode != 2 && opcode != 6) || size == 3 {
        return false;
    }
    let e = 1usize << size;
    let we = e * 2;
    let lanes = (if q { 16 } else { 8 }) / e;
    let n = ctx.q[rn];
    let d = ctx.q[rd];
    let wm = mk(we);
    let mut out = [0u8; 16];
    for i in 0..lanes / 2 {
        let a = rl(&n, e, 2 * i);
        let b = rl(&n, e, 2 * i + 1);
        let sum: i128 = if u { a as i128 + b as i128 } else { sx(a, e) as i128 + sx(b, e) as i128 };
        let mut v = (sum as u64) & wm;
        if opcode == 6 {
            v = v.wrapping_add(rl(&d, we, i)) & wm;
        }
        wl(&mut out, we, i, v);
    }
    ctx.q[rd] = out;
    true
}

fn pairwise3(ctx: &mut Ctx, w: u32) -> bool {
    let q = field(w, 30, 30) == 1;
    let u = field(w, 29, 29) == 1;
    let size = field(w, 23, 22) as usize;
    let rm = field(w, 20, 16) as usize;
    let opcode = field(w, 15, 11);
    let rn = field(w, 9, 5) as usize;
    let rd = field(w, 4, 0) as usize;
    let ok = (opcode == 0x17 && !u) || opcode == 0x14 || opcode == 0x15;
    if !ok || (size == 3 && (!q || opcode != 0x17)) {
        return false;
    }
    let e = 1usize << size;
    let lanes = (if q { 16 } else { 8 }) / e;
    let n = ctx.q[rn];
    let m = ctx.q[rm];
    let wm = mk(e);
    let mut out = [0u8; 16];
    for i in 0..lanes {
        let (src, j) = if i < lanes / 2 { (&n, 2 * i) } else { (&m, 2 * (i - lanes / 2)) };
        let a = rl(src, e, j);
        let b = rl(src, e, j + 1);
        let r = match opcode {
            0x17 => a.wrapping_add(b) & wm,
            0x14 => {
                if u {
                    a.max(b)
                } else if sx(a, e) >= sx(b, e) {
                    a
                } else {
                    b
                }
            }
            _ => {
                if u {
                    a.min(b)
                } else if sx(a, e) <= sx(b, e) {
                    a
                } else {
                    b
                }
            }
        };
        wl(&mut out, e, i, r);
    }
    ctx.q[rd] = out;
    true
}
