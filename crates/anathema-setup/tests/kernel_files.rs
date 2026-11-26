use anathema_setup::fat32::FileSpec;
use anathema_setup::kernel_files::{self, esp_files, kc_config, prepare_with, space_check, update_plan, KernelFiles};
use std::path::{Path, PathBuf};

fn temp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("anathema-kf-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn space_check_reports_need_and_free() {
    let err = space_check(1_000_000_000, 3_600_000_000, Path::new("C:\\Users\\x\\AppData\\Local\\Anathema\\kdk")).unwrap_err();
    assert!(err.contains("3.6 GB needed, 1.0 GB free"), "{err}");
    assert!(err.contains("Anathema\\kdk"), "{err}");
    assert!(space_check(4_000_000_000, 3_600_000_000, Path::new("C:\\")).is_ok());
}

/// Writes a finished cache: out/xkernel, out/xkc.bin and a stamp recording their hashes.
fn seed_cache(cache: &Path, stamp: &str) {
    let out = cache.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("xkernel"), b"kernel bytes").unwrap();
    std::fs::write(out.join("xkc.bin"), b"kc bytes").unwrap();
    kernel_files::write_stamp(&out, stamp, b"kernel bytes", b"kc bytes").unwrap();
}

fn refusing_fetch() -> impl FnMut(&anathema_media::kdk::pin::Kdk, &Path, &mut dyn FnMut(u64, u64)) -> Result<(), String> {
    |k, _, _| Err(format!("fetch called for {}", k.build))
}

#[test]
fn stamp_mismatch_forces_rebuild() {
    let cache = temp("stale");
    seed_cache(&cache, "an older setup");
    let err = prepare_with(&cache, u64::MAX, &mut refusing_fetch(), &mut |_, _| {}, &mut |_| {}).err().unwrap();
    assert_eq!(err, "fetch called for 25G342");
}

#[test]
fn stamp_match_uses_cache() {
    let cache = temp("fresh");
    seed_cache(&cache, &kernel_files::stamp());
    let k = prepare_with(&cache, u64::MAX, &mut refusing_fetch(), &mut |_, _| {}, &mut |_| {}).unwrap();
    assert_eq!((k.xkernel.as_slice(), k.xkc.as_slice()), (&b"kernel bytes"[..], &b"kc bytes"[..]));
}

#[test]
fn corrupt_cache_forces_rebuild() {
    let cache = temp("corrupt");
    seed_cache(&cache, &kernel_files::stamp());
    std::fs::write(cache.join("out/xkc.bin"), b"kc bytez").unwrap();
    assert!(prepare_with(&cache, u64::MAX, &mut refusing_fetch(), &mut |_, _| {}, &mut |_| {}).is_err());
}

#[test]
fn no_space_stops_before_any_download() {
    let cache = temp("nospace");
    let err = prepare_with(&cache, 1_000_000, &mut refusing_fetch(), &mut |_, _| {}, &mut |_| {}).err().unwrap();
    assert!(err.contains("Not enough free space"), "{err}");
}

#[test]
fn update_plan_rejects_full_esp() {
    let big = vec![0u8; 60 << 20];
    let files = [FileSpec { path: "anathema/xkc.bin", data: &big }];
    let err = update_plan(10 << 20, &[], &files).unwrap_err();
    assert!(err.contains("too small"), "{err}");
    // replacing a file of the same size still needs room for the new copy while both exist
    assert!(update_plan(10 << 20, &[("anathema/xkc.bin".into(), 60 << 20)], &files).is_err());
    assert!(update_plan(70 << 20, &[("anathema/xkc.bin".into(), 60 << 20)], &files).is_ok());
}

#[test]
fn update_plan_counts_space_freed_by_earlier_replacements() {
    let a = vec![0u8; 30 << 20];
    let b = vec![0u8; 30 << 20];
    let files = [FileSpec { path: "anathema/xkernel", data: &a }, FileSpec { path: "anathema/xkc.bin", data: &b }];
    // 35 MB free: xkernel's new copy fits, then its old 30 MB copy is freed before xkc.bin
    let existing = [("anathema/xkernel".to_string(), 30u64 << 20), ("anathema/xkc.bin".to_string(), 30 << 20)];
    assert!(update_plan(35 << 20, &existing, &files).is_ok());
    assert!(update_plan(35 << 20, &[], &files).is_err());
}

#[test]
fn esp_files_paths() {
    let k = KernelFiles { xkernel: vec![1], xkc: vec![2] };
    let paths: Vec<&str> = esp_files(b"efi", &k).iter().map(|f| f.path).collect();
    assert_eq!(
        paths,
        ["EFI/BOOT/BOOTX64.EFI", "anathema/xkernel", "anathema/xkc.bin", "anathema/xcp.bin", "anathema/xcp2.bin", "anathema/xcp3.bin"]
    );
    let files = esp_files(b"efi", &k);
    assert_eq!(files[3].data, anathema_setup::XCP);
    assert_eq!(files[4].data, anathema_setup::XCP2);
    assert_eq!(files[5].data, anathema_setup::XCP3);
}

/// The size limits are the ones anathema-xboot checks before it installs each stub.
#[test]
fn stubs_fit_the_loader() {
    use anathema_setup::{XCP, XCP2, XCP3};
    // tools/mkxcp.sh builds with -Os, the same 496 bytes the commpage_stub.c build gives
    assert_eq!(XCP.len(), 0x1f0);
    assert!((0x40..=0x3c0).contains(&XCP2.len()), "xcp2.bin is {:#x} bytes", XCP2.len());
    assert!((0x40..=0x700).contains(&XCP3.len()), "xcp3.bin is {:#x} bytes", XCP3.len());
    // xboot patches this placeholder with the address of the stub's store
    let store = 0x1122_3344_5566_7788u64.to_le_bytes();
    assert!(XCP2.windows(8).any(|w| w == store));
}

/// xcp3.bin starts kexts by bundle id, so each one has to be in the kext collection.
#[test]
fn kexts_xcp3_starts_are_linked() {
    let c = kc_config(Path::new("T"), Path::new("S"), Path::new("K"));
    let ids: Vec<String> = anathema_setup::XCP3
        .split(|&b| b == 0)
        .filter_map(|s| std::str::from_utf8(s).ok())
        .filter_map(|s| s.find("com.apple.").map(|i| s[i..].to_string()))
        .collect();
    assert_eq!(
        ids,
        [
            "com.apple.filesystems.tmpfs",
            "com.apple.AppleFSCompression.AppleFSCompressionTypeZlib",
            "com.apple.AppleFSCompression.AppleFSCompressionTypeDataless",
        ]
    );
    for id in &ids {
        assert!(c.roots.contains(id), "{id} is not a kext collection root");
    }
}

#[test]
fn kc_config_matches_mkkc() {
    let c = kc_config(Path::new("T"), Path::new("S"), Path::new("K"));
    assert_eq!(c.ext, [PathBuf::from("T"), PathBuf::from("S")]);
    assert_eq!(c.kernel, PathBuf::from("K"));
    assert_eq!(c.base, 0xffff_ff80_0200_0000);
    assert_eq!(
        &c.roots[..9],
        [
            "com.apple.driver.AppleACPIPlatform", "com.apple.kec.corecrypto", "com.apple.kec.AppleEncryptedArchive",
            "com.apple.kec.XrtHostedXnu", "com.apple.kec.Libm", "com.apple.kec.Compression", "com.apple.kec.pthread",
            "com.apple.security.AppleImage4", "com.apple.kext.CoreTrust",
        ]
    );
    let file_roots: Vec<&str> = include_str!("../../../tools/kc-roots.txt").lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    assert_eq!(&c.roots[9..], file_roots.as_slice());
    assert_eq!(
        c.prefer_last,
        [
            "com.apple.kec.corecrypto", "com.apple.kec.Compression", "com.apple.kec.pthread", "com.apple.kec.Libm",
            "com.apple.kec.AppleEncryptedArchive", "com.apple.kec.XrtHostedXnu", "com.apple.security.AppleImage4",
        ]
    );
    assert_eq!(
        c.strip,
        [
            ("com.apple.security.AppleImage4".to_string(), "AppleSecurityExtension".to_string()),
            ("com.apple.kext.CoreTrust".to_string(), "AppleSecurityExtension".to_string()),
        ]
    );
    assert_eq!(c.codeless, ["com.apple.driver.AppleMobileFileIntegrity"]);
    assert!(c.no_start.is_empty() && c.set_true.is_empty() && c.rename.is_empty() && c.add_lib.is_empty() && c.overrides.is_empty());
}

mod review_fixes {
    use super::*;
    use anathema_media::kdk::extract::MARKER;
    use anathema_media::kdk::pin::{KDK_27, KDK_TAHOE};
    use anathema_setup::kernel_files::{space_needed, stamp_inputs, update_order, PIN_FILE};
    use anathema_setup::stick::{same_stick, StickId};

    fn mark(tree: &Path, sha: &str) {
        std::fs::create_dir_all(tree).unwrap();
        std::fs::write(tree.join(MARKER), b"files 1\nbytes 1\n").unwrap();
        std::fs::write(tree.join(PIN_FILE), sha).unwrap();
    }

    #[test]
    fn stamp_covers_the_linker_source() {
        let text = stamp_inputs();
        let line = text.lines().find(|l| l.starts_with("linker-source ")).expect("linker source line");
        let hash = line.trim_start_matches("linker-source ");
        assert_eq!(hash.len(), 16, "{line}");
        assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()), "{line}");
    }

    #[test]
    fn tree_from_another_pin_is_unpacked_again() {
        let cache = temp("otherpin");
        mark(&cache.join(KDK_TAHOE.build), "an older sha256");
        mark(&cache.join(KDK_27.build), KDK_27.sha256);
        let err = prepare_with(&cache, u64::MAX, &mut refusing_fetch(), &mut |_, _| {}, &mut |_| {}).err().unwrap();
        assert_eq!(err, "fetch called for 25G342");
    }

    #[test]
    fn space_needed_starts_near_the_peak() {
        let need = space_needed(&temp("space-fresh"));
        assert!((3_300_000_000..=3_800_000_000).contains(&need), "{need}");
    }

    #[test]
    fn space_needed_counts_what_is_already_cached() {
        let cache = temp("space-resume");
        mark(&cache.join(KDK_TAHOE.build), KDK_TAHOE.sha256);
        let part = std::fs::File::create(cache.join(format!("{}.dmg.part", KDK_27.build))).unwrap();
        part.set_len(KDK_27.size / 2).unwrap();
        let need = space_needed(&cache);
        assert!(need < 2_200_000_000, "{need}");
        // the reviewer's case: 2.4 GB free goes on to the download instead of refusing
        let err = prepare_with(&cache, 2_400_000_000, &mut refusing_fetch(), &mut |_, _| {}, &mut |_| {}).err().unwrap();
        assert_eq!(err, "fetch called for 26B131");
    }

    #[test]
    fn update_writes_the_loader_last() {
        let k = KernelFiles { xkernel: vec![1], xkc: vec![2] };
        let files = esp_files(b"efi", &k);
        let order: Vec<&str> = update_order(&files).iter().map(|f| f.path).collect();
        assert_eq!(
            order,
            ["anathema/xkernel", "anathema/xkc.bin", "anathema/xcp.bin", "anathema/xcp2.bin", "anathema/xcp3.bin", "EFI/BOOT/BOOTX64.EFI"]
        );
    }

    fn id(model: &str, bytes: u64, parts: &[(u64, u64)]) -> StickId {
        StickId { model: model.into(), bytes, bus: 7, parts: parts.iter().map(|&(o, l)| (o, l, [0u8; 16])).collect() }
    }

    #[test]
    fn same_stick_accepts_the_same_disk() {
        let a = id("SanDisk Ultra", 32_000_000_000, &[(1 << 20, 200 << 20)]);
        assert!(same_stick(&a, &a.clone()).is_ok());
    }

    #[test]
    fn same_stick_rejects_a_swapped_disk() {
        let a = id("SanDisk Ultra", 32_000_000_000, &[(1 << 20, 200 << 20)]);
        for b in [
            id("Kingston DataTraveler", 32_000_000_000, &[(1 << 20, 200 << 20)]),
            id("SanDisk Ultra", 31_000_000_000, &[(1 << 20, 200 << 20)]),
            id("SanDisk Ultra", 32_000_000_000, &[(1 << 20, 100 << 20)]),
            id("SanDisk Ultra", 32_000_000_000, &[]),
        ] {
            let err = same_stick(&a, &b).unwrap_err();
            assert!(err.contains("changed"), "{err}");
        }
    }
}
