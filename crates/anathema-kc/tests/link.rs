mod support;

use anathema_kc::{link, Config};
use anathema_media::plist::{self, Value};
use std::fs;
use std::path::{Path, PathBuf};
use support::*;

const BASE: u64 = 0xffff_ff80_0200_0000;

fn rd64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn rd32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// Writes `<root>/<name>.kext` with an Info.plist and, when given, Contents/MacOS/<name>.
fn bundle(root: &Path, name: &str, id: &str, exe: Option<&[u8]>, libs: &[&str], extra: &str) -> PathBuf {
    let dir = root.join(format!("{name}.kext"));
    fs::create_dir_all(dir.join("Contents/MacOS")).unwrap();
    let libs: String = libs.iter().map(|l| format!("<key>{l}</key><string>1.0</string>")).collect();
    let exe_key = if exe.is_some() { format!("<key>CFBundleExecutable</key><string>{name}</string>") } else { String::new() };
    let info = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{id}</string>{exe_key}<key>OSBundleLibraries</key><dict>{libs}</dict>{extra}</dict></plist>\n"
    );
    fs::write(dir.join("Contents/Info.plist"), info).unwrap();
    if let Some(bin) = exe {
        fs::write(dir.join("Contents/MacOS").join(name), bin).unwrap();
    }
    dir
}

fn kernel(dir: &Path) -> PathBuf {
    let k = build_kext(&KextSpec { exports: vec![("_kernel_fn", 0xffff_ff80_0010_0000)], ..Default::default() });
    let p = dir.join("kernel");
    fs::write(&p, k).unwrap();
    p
}

fn cfg(dir: &Path, ext: Vec<PathBuf>, roots: &[&str]) -> Config {
    Config {
        kernel: kernel(dir),
        ext,
        base: BASE,
        roots: roots.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

/// Offset in the xkc.bin file of image offset `at` in the kext loaded at `load`.
fn at(load: u64, at: u64) -> usize {
    (64 + load - BASE + at) as usize
}

fn map_load(map: &str, id: &str) -> u64 {
    let line = map.lines().find(|l| l.starts_with(&format!("{id} "))).expect("kext in map");
    u64::from_str_radix(line.split(' ').nth(1).unwrap().trim_start_matches("0x"), 16).unwrap()
}

fn infos(xkc: &[u8]) -> Vec<Value> {
    let off = rd64(xkc, 24) as usize + 64;
    let len = rd64(xkc, 32) as usize;
    let text = std::str::from_utf8(&xkc[off..off + len - 1]).unwrap();
    plist::parse(text).unwrap().get("_PrelinkInfoDictionary").unwrap().as_array().unwrap().to_vec()
}

fn info_for<'a>(all: &'a [Value], id: &str) -> &'a Value {
    all.iter().find(|v| v.get("CFBundleIdentifier").and_then(Value::as_str) == Some(id)).expect("record")
}

fn spec_a() -> KextSpec {
    KextSpec { exports: vec![("_foo", 0x1010)], data: vec![0; 0x200], kmod_info_at: Some(0x4000), ..Default::default() }
}

fn spec_b(imports: Vec<&'static str>) -> KextSpec {
    KextSpec { imports, data: vec![0; 0x200], kmod_info_at: Some(0x4000), ..Default::default() }
}

#[test]
fn links_two_kexts_and_resolves_dependency() {
    let dir = temp_dir("dep");
    let ext = dir.join("ext");
    bundle(&ext, "A", "com.test.a", Some(&build_kext(&spec_a())), &[], "");
    let mut b = spec_b(vec!["_foo", "_kernel_fn"]);
    b.data[0x08..0x10].copy_from_slice(&8u64.to_le_bytes());
    b.ext_relocs = vec![(0x4008, b.import_index("_foo"), 0, 3, false), (0x4010, b.import_index("_kernel_fn"), 0, 3, false)];
    bundle(&ext, "B", "com.test.b", Some(&build_kext(&b)), &["com.test.a"], "");
    let out = link(&cfg(&dir, vec![ext], &["com.test.b"])).unwrap();
    let (la, lb) = (map_load(&out.map, "com.test.a"), map_load(&out.map, "com.test.b"));
    assert_eq!(la, BASE, "dependencies link first");
    assert_eq!(lb, BASE + 0x8000);
    assert_eq!(rd64(&out.xkc, at(lb, 0x4008)), la + 0x1010 + 8, "S + addend");
    assert_eq!(rd64(&out.xkc, at(lb, 0x4010)), 0xffff_ff80_0010_0000, "kernel symbol");
}

#[test]
fn branch_reloc_is_pc_relative() {
    let dir = temp_dir("branch");
    let ext = dir.join("ext");
    bundle(&ext, "A", "com.test.a", Some(&build_kext(&spec_a())), &[], "");
    let mut b = spec_b(vec!["_foo"]);
    b.text = vec![0xe8, 0, 0, 0, 0, 0xc3];
    b.ext_relocs = vec![(0x1001, b.import_index("_foo"), 2, 2, true)];
    bundle(&ext, "B", "com.test.b", Some(&build_kext(&b)), &["com.test.a"], "");
    let out = link(&cfg(&dir, vec![ext], &["com.test.b"])).unwrap();
    let (la, lb) = (map_load(&out.map, "com.test.a"), map_load(&out.map, "com.test.b"));
    let got = i32::from_le_bytes(out.xkc[at(lb, 0x1001)..at(lb, 0x1001) + 4].try_into().unwrap()) as i64;
    assert_eq!(got, (la + 0x1010) as i64 - (lb + 0x1001 + 4) as i64);
}

#[test]
fn local_reloc_adds_load_address() {
    let dir = temp_dir("local");
    let ext = dir.join("ext");
    let mut a = spec_a();
    a.data[0x10..0x18].copy_from_slice(&0x1234u64.to_le_bytes());
    a.local_relocs = vec![0x4010];
    bundle(&ext, "A", "com.test.a", Some(&build_kext(&a)), &[], "");
    let out = link(&cfg(&dir, vec![ext], &["com.test.a"])).unwrap();
    assert_eq!(rd64(&out.xkc, at(BASE, 0x4010)), BASE + 0x1234);
}

#[test]
fn unresolved_symbol_points_at_stub_and_is_reported() {
    let dir = temp_dir("unres");
    let ext = dir.join("ext");
    let mut b = spec_b(vec!["_nope"]);
    b.ext_relocs = vec![(0x4008, b.import_index("_nope"), 0, 3, false)];
    bundle(&ext, "B", "com.test.b", Some(&build_kext(&b)), &[], "");
    let out = link(&cfg(&dir, vec![ext], &["com.test.b"])).unwrap();
    let total = rd64(&out.xkc, 16);
    assert_eq!(rd64(&out.xkc, at(BASE, 0x4008)), BASE + total + 0x80000 + 0x1800);
    let text = out.report.to_string();
    assert!(text.contains("UNRESOLVED symbols (1):\n   _nope ['com.test.b']\n"), "{text}");
}

#[test]
fn kmod_info_gets_each_kexts_own_size() {
    let dir = temp_dir("kmod");
    let ext = dir.join("ext");
    let a = spec_a();
    let b = KextSpec { data: vec![0; 0x4100], ..spec_b(vec![]) };
    let (abin, bbin) = (build_kext(&a), build_kext(&b));
    bundle(&ext, "A", "com.test.a", Some(&abin), &[], "");
    bundle(&ext, "B", "com.test.b", Some(&bbin), &["com.test.a"], "");
    let out = link(&cfg(&dir, vec![ext], &["com.test.b"])).unwrap();
    for (id, bin, size) in [("com.test.a", &abin, 0x8000u64), ("com.test.b", &bbin, 0xc000)] {
        let load = map_load(&out.map, id);
        let hdr_size = 32 + rd32(bin, 20) as u64;
        let k = at(load, 0x4000 + 156);
        assert_eq!((rd64(&out.xkc, k), rd64(&out.xkc, k + 8), rd64(&out.xkc, k + 16)), (load, size, hdr_size), "{id}");
        // third segment command is __LINKEDIT: vmaddr = load + own size, the rest zero
        let lc = 32 + 2 * (72 + 80);
        assert_eq!(rd64(&out.xkc, at(load, lc as u64 + 24)), load + size, "{id} linkedit");
        assert_eq!(rd32(&out.xkc, at(load, 24)) & 0x8000_0000, 0x8000_0000, "{id} MH_DYLIB_IN_CACHE");
    }
}

#[test]
fn header_layout() {
    let dir = temp_dir("hdr");
    let ext = dir.join("ext");
    bundle(&ext, "A", "com.test.a", Some(&build_kext(&spec_a())), &[], "");
    let out = link(&cfg(&dir, vec![ext.clone()], &["com.test.a"])).unwrap();
    let x = &out.xkc;
    let (magic, base, total, poff, plen, cc_start, cc_kmod, blen) =
        (rd64(x, 0), rd64(x, 8), rd64(x, 16), rd64(x, 24), rd64(x, 32), rd64(x, 40), rd64(x, 48), rd64(x, 56));
    assert_eq!((magic, base, total, poff), (0x434b_5841_4e41, BASE, 0x8000, 0x8000));
    assert_eq!((cc_start, cc_kmod), (0, 0));
    assert_eq!(blen, poff + 0x80000 + 0x2000);
    assert_eq!(x.len() as u64, 64 + blen);
    let p = &x[64 + poff as usize..64 + (poff + plen) as usize];
    assert!(p.starts_with(b"<?xml version=\"1.0\""));
    assert_eq!(p.last(), Some(&0));
    assert_eq!(out.map, format!("com.test.a {BASE:#x} 0x8000 {}\n", ext.join("A.kext").join("Contents").join("MacOS").join("A").display()));
    let text = out.report.to_string();
    assert!(text.starts_with("kexts selected: 1 (with code: 1)\nblock 0x8000 bytes, plist "), "{text}");
    assert!(text.contains("relocs: {'ext64': 0, 'branch': 0, 'rebase': 0}\ncorecrypto start 0x0 kmod 0x0\n"), "{text}");
}

#[test]
fn prelink_keys_and_codeless_and_strip_edit_plist() {
    let dir = temp_dir("plist");
    let ext = dir.join("ext");
    bundle(&ext, "A", "com.test.a", Some(&build_kext(&spec_a())), &[], "<key>Drop</key><string>x</string><key>IOKitPersonalities</key><dict/>");
    bundle(&ext, "C", "com.test.c", Some(&build_kext(&spec_a())), &[], "<key>IOKitPersonalities</key><dict/>");
    bundle(&ext, "R", "com.test.r", None, &[], "<key>OSKernelResource</key><true/>");
    let mut c = cfg(&dir, vec![ext], &["com.test.a", "com.test.c"]);
    c.codeless = vec!["com.test.c".into()];
    c.strip = vec![("com.test.a".into(), "Drop".into())];
    c.set_true = vec![("com.test.a".into(), "OSBundleRequired".into())];
    let out = link(&c).unwrap();
    let all = infos(&out.xkc);
    assert_eq!(all.len(), 3, "roots plus the kernel resource");
    let a = info_for(&all, "com.test.a");
    assert!(a.get("Drop").is_none());
    assert_eq!(a.get("OSBundleRequired").and_then(Value::as_bool), Some(true));
    assert_eq!(a.get("_PrelinkBundlePath").and_then(Value::as_str), Some("/System/Library/Extensions/A.kext"));
    assert_eq!(a.get("_PrelinkExecutableRelativePath").and_then(Value::as_str), Some("Contents/MacOS/A"));
    assert_eq!(a.get("_PrelinkExecutableLoadAddr").and_then(Value::as_int), Some(BASE as i64));
    assert_eq!(a.get("_PrelinkExecutableSize").and_then(Value::as_int), Some(0x8000));
    assert_eq!(a.get("_PrelinkKmodInfo").and_then(Value::as_int), Some((BASE + 0x4000) as i64));
    let cr = info_for(&all, "com.test.c");
    assert!(cr.get("CFBundleExecutable").is_none() && cr.get("IOKitPersonalities").is_none());
    assert!(cr.get("_PrelinkExecutableLoadAddr").is_none());
    assert!(info_for(&all, "com.test.r").get("_PrelinkExecutableLoadAddr").is_none());
    assert!(!out.map.contains("com.test.c"));
}

#[test]
fn prefer_last_takes_bundle_from_last_ext() {
    let dir = temp_dir("prefer");
    let (e1, e2) = (dir.join("e1"), dir.join("e2"));
    bundle(&e1, "A", "com.test.a", Some(&build_kext(&spec_a())), &[], "<key>From</key><string>first</string>");
    bundle(&e2, "A", "com.test.a", Some(&build_kext(&spec_a())), &[], "<key>From</key><string>last</string>");
    let first = link(&cfg(&dir, vec![e1.clone(), e2.clone()], &["com.test.a"])).unwrap();
    assert_eq!(info_for(&infos(&first.xkc), "com.test.a").get("From").and_then(Value::as_str), Some("first"));
    let mut c = cfg(&dir, vec![e1, e2], &["com.test.a"]);
    c.prefer_last = vec!["com.test.a".into()];
    let last = link(&c).unwrap();
    assert_eq!(info_for(&infos(&last.xkc), "com.test.a").get("From").and_then(Value::as_str), Some("last"));
}

#[test]
fn unhandled_reloc_kind_is_an_error() {
    let dir = temp_dir("badreloc");
    let ext = dir.join("ext");
    let mut b = spec_b(vec!["_kernel_fn"]);
    b.ext_relocs = vec![(0x4008, b.import_index("_kernel_fn"), 1, 2, false)];
    bundle(&ext, "B", "com.test.b", Some(&build_kext(&b)), &[], "");
    let err = link(&cfg(&dir, vec![ext], &["com.test.b"])).unwrap_err().to_string();
    assert_eq!(err, "unhandled ext reloc (1, 2, 0) in com.test.b");
}
