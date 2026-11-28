#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::arch::asm;
use uefi::boot::{self, AllocateType, MemoryType};
use uefi::fs::FileSystem;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::{cstr16, println};

const KBASE: u64 = 0xffff_ff80_0000_0000;
const DEFAULT_ARGS: &str = "-v serial=3 debug=0x14e cpus=1 kt_fixed_zones=200 kt_var_heaps=6";

struct Patch {
    va: u64,
    expect: &'static [u8],
    replace: &'static [u8],
    why: &'static str,
}




// applied only when \\anathema\\xpatch-amd exists: the kernel's cpu identification refuses anything
// that is not an intel cpu ("Unsupported CPU" in cpuid_set_info).
const AMD_PATCHES: &[Patch] = &[
    Patch {
        va: 0xffffff80003e80ee,
        expect: &[0x8b, 0x88, 0xdc, 0x00, 0x00, 0x00],
        replace: &[0xb9, 0x01, 0x00, 0x00, 0x00, 0x90],
        why: "cpu topology: the kernel is told 1 thread per package, so the shared cache must say 1 too or it divides by zero",
    },
    Patch {
        va: 0xffffff80003e8102,
        expect: &[0x8b, 0x88, 0xe0, 0x00, 0x00, 0x00],
        replace: &[0xb9, 0x01, 0x00, 0x00, 0x00, 0x90],
        why: "cpu topology: the kernel is told 1 thread per package, so the shared cache must say 1 too or it divides by zero",
    },
    Patch {
        va: 0xffff_ff80_003e_589b,
        expect: &[0x0f, 0x85, 0x04, 0x03, 0x00, 0x00],
        replace: &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
        why: "cpuid_set_info vendor check",
    },
    // the cache line size comes from intel's cpuid leaf 4, which amd cpus don't have: use 64
    Patch {
        va: 0xffff_ff80_003e_6567,
        expect: &[0x8b, 0x5d, 0xbc, 0x85, 0xdb, 0x75, 0x0b, 0x8b, 0x5d, 0xb8, 0x85, 0xdb],
        replace: &[0xbb, 0x40, 0x00, 0x00, 0x00, 0xeb, 0x0b, 0x90, 0x90, 0x90, 0x90, 0x90],
        why: "cpuid cache line size fallback",
    },
    Patch {
        va: 0xffff_ff80_003e_5e00,
        expect: &[0x55],
        replace: &[0xc3],
        why: "cpuid mitigation setup uses intel only tsx/spec ctrl msrs, return at once",
    },
    Patch {
        va: 0xffffff800041bc20,
        expect: &[0x55],
        replace: &[0xc3],
        why: "xcpm_bootstrap: intel xcpm power management (rapl, hwp, tsc deadline msrs) does not exist on amd, return at once",
    },
    Patch {
        va: 0xffffff800041c700,
        expect: &[0x55],
        replace: &[0xc3],
        why: "xcpm_init: intel xcpm power management (rapl, hwp, tsc deadline msrs) does not exist on amd, return at once",
    },
    Patch {
        va: 0xffffff8000401719,
        expect: &[0xb9, 0x98, 0x01, 0x00, 0x00, 0x0f, 0x32],
        replace: &[0x31, 0xc0, 0x31, 0xd2, 0x90, 0x90, 0x90],
        why: "tsc_init: rdmsr 0x198 is intel only and faults on amd, return zero (tsc ratio is patched in separately)",
    },
    Patch {
        va: 0xffffff8000401755,
        expect: &[0xb9, 0x94, 0x01, 0x00, 0x00, 0x0f, 0x32],
        replace: &[0x31, 0xc0, 0x31, 0xd2, 0x90, 0x90, 0x90],
        why: "tsc_init: rdmsr 0x194 is intel only and faults on amd, return zero (tsc ratio is patched in separately)",
    },
    Patch {
        va: 0xffffff800040175e,
        expect: &[0xb9, 0xce, 0x00, 0x00, 0x00, 0x0f, 0x32],
        replace: &[0x31, 0xc0, 0x31, 0xd2, 0x90, 0x90, 0x90],
        why: "tsc_init: rdmsr 0xce is intel only and faults on amd, return zero (tsc ratio is patched in separately)",
    },
    Patch {
        va: 0xffffff80003e35c4,
        expect: &[0x0f, 0x30],
        replace: &[0x66, 0x90],
        why: "cpu_syscall_init: wrmsr 0x174 SYSENTER_CS faults on amd (64 bit sysenter address), unused on x86_64",
    },
    Patch {
        va: 0xffffff80003e35fe,
        expect: &[0x0f, 0x30],
        replace: &[0x66, 0x90],
        why: "cpu_syscall_init: wrmsr 0x176 SYSENTER_EIP faults on amd (64 bit sysenter address), unused on x86_64",
    },
    Patch {
        va: 0xffffff80003e361b,
        expect: &[0x0f, 0x30],
        replace: &[0x66, 0x90],
        why: "cpu_syscall_init: wrmsr 0x175 SYSENTER_ESP faults on amd (64 bit sysenter address), unused on x86_64",
    },
    Patch {
        va: 0xffff_ff80_003e_5a89,
        expect: &[0x0f, 0x32, 0x09, 0xc2],
        replace: &[0x31, 0xc0, 0x31, 0xd2],
        why: "core thread count msr 0x35 is intel only, report 1 core 1 thread",
    },
    Patch {
        va: 0xffff_ff80_003e_5aa5,
        expect: &[0x0f, 0x32, 0x09, 0xc2],
        replace: &[0x31, 0xc0, 0x31, 0xd2],
        why: "core thread count msr 0x35 is intel only, report 1 core 1 thread",
    },
    Patch {
        va: 0xffffff8000401ffc,
        expect: &[0xb9, 0xa0, 0x01, 0x00, 0x00, 0x0f, 0x32],
        replace: &[0x66, 0x90, 0x66, 0x90, 0x66, 0x90, 0x90],
        why: "commpage_populate: remove rdmsr 0x1a0 (amd_vanilla)",
    },
    Patch {
        va: 0xffffff80003e65e3,
        expect: &[0xb8, 0x04, 0x00, 0x00, 0x00, 0x44, 0x89, 0xf1, 0x44, 0x89],
        replace: &[0xb8, 0x1d, 0x00, 0x00, 0x80, 0x44, 0x89, 0xf1, 0x44, 0x89],
        why: "cpuid_set_cache_info: use leaf 0x8000001d instead of 4 (amd_vanilla)",
    },
    Patch {
        va: 0xffffff80003e4ac5,
        expect: &[0xb9, 0x8b, 0x00, 0x00, 0x00, 0x31, 0xc0, 0x31, 0xd2, 0x0f, 0x30],
        replace: &[0x66, 0x90, 0x66, 0x90, 0x66, 0x90, 0x66, 0x90, 0x66, 0x90, 0x90],
        why: "cpuid_set_generic_info: remove wrmsr 0x8b (amd_vanilla)",
    },
    Patch {
        va: 0xffffff80003e4b0d,
        expect: &[0xb9, 0x8b, 0x00, 0x00, 0x00, 0x0f, 0x32],
        replace: &[0xba, 0xba, 0x00, 0x00, 0x00, 0x66, 0x90],
        why: "cpuid_set_generic_info: rdmsr 0x8b -> constant (amd_vanilla)",
    },
    Patch {
        va: 0xffffff80003e4b85,
        expect: &[0xb9, 0x17, 0x00, 0x00, 0x00, 0x0f, 0x32, 0xc1, 0xea, 0x12, 0x80, 0xe2, 0x07],
        replace: &[0xb2, 0x01, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x90],
        why: "cpuid_set_generic_info: platform id flag = 1 (amd_vanilla)",
    },
    Patch {
        va: 0xffffff80003e4ec4,
        expect: &[0x00, 0x05, 0x0f, 0x82],
        replace: &[0x00, 0x00, 0x0f, 0x82],
        why: "cpuid_set_generic_info: allow leaf 7 (amd_vanilla)",
    },
    Patch {
        va: 0xffffff80003e58a5,
        expect: &[0x80, 0x3d, 0xa0, 0x88, 0xcd, 0x00, 0x06, 0x75],
        replace: &[0xba, 0xbc, 0x4f, 0xea, 0x78, 0x31, 0xdb, 0xeb],
        why: "cpuid_set_cpufamily: force PENRYN (amd_vanilla)",
    },
    Patch {
        va: 0xffffff80003f04b4,
        expect: &[0xb9, 0x99, 0x01, 0x00, 0x00, 0x0f, 0x32, 0x48, 0xc1, 0xe2, 0x20, 0x89, 0xc6, 0x48, 0x09, 0xd6, 0xb9, 0x98, 0x01, 0x00, 0x00, 0x0f, 0x32, 0x48, 0xc1, 0xe2, 0x20, 0x89, 0xc0, 0x48, 0x09, 0xc2, 0xbf, 0x58, 0x02, 0x31, 0x05, 0x31, 0xc9, 0x45, 0x31, 0xc0],
        replace: &[0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x44, 0x00, 0x00],
        why: "i386_vm_init: remove 3 rdmsr (amd_vanilla)",
    },
    Patch {
        va: 0xffffff8000b0290b,
        expect: &[0xb9, 0x99, 0x01, 0x00, 0x00, 0x0f, 0x32, 0x48, 0xc1, 0xe2, 0x20, 0x89, 0xc6, 0x48, 0x09, 0xd6, 0xb9, 0x98, 0x01, 0x00, 0x00, 0x0f, 0x32, 0x48, 0xc1, 0xe2, 0x20, 0x89, 0xc0, 0x48, 0x09, 0xc2, 0xbf, 0x58, 0x02, 0x31, 0x05, 0x31, 0xc9, 0x45, 0x31, 0xc0],
        replace: &[0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x44, 0x00, 0x00],
        why: "i386_init: remove 3 rdmsr (amd_vanilla)",
    },
    Patch {
        va: 0xffffff8000b02deb,
        expect: &[0xb9, 0x99, 0x01, 0x00, 0x00, 0x0f, 0x32, 0x48, 0xc1, 0xe2, 0x20, 0x89, 0xc6, 0x48, 0x09, 0xd6, 0xb9, 0x98, 0x01, 0x00, 0x00, 0x0f, 0x32, 0x48, 0xc1, 0xe2, 0x20, 0x89, 0xc0, 0x48, 0x09, 0xc2, 0xbf, 0x58, 0x02, 0x31, 0x05, 0x31, 0xc9, 0x45, 0x31, 0xc0],
        replace: &[0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x0f, 0x1f, 0x44, 0x00, 0x00],
        why: "i386_init: remove 3 rdmsr #2 (amd_vanilla)",
    },
    Patch {
        va: 0xffffff800040645a,
        expect: &[0x25, 0xfc, 0x00, 0x00, 0x00, 0x83, 0xf8, 0x13],
        replace: &[0x25, 0xfc, 0x00, 0x00, 0x00, 0x0f, 0x1f, 0x00],
        why: "remove version check and panic (amd_vanilla)",
    },
    Patch {
        va: 0xffffff800040dcba,
        expect: &[0x89, 0xc0, 0x81, 0xe2, 0xff, 0xff, 0x00, 0xff, 0x81, 0xca, 0x00, 0x00, 0x01, 0x00, 0x0f, 0x30],
        replace: &[0x89, 0xc0, 0xb8, 0x06, 0x01, 0x07, 0x00, 0xba, 0x06, 0x01, 0x07, 0x00, 0x0f, 0x30, 0x90, 0x90],
        why: "mtrr_update_action: fix PAT (amd_vanilla)",
    },
    Patch {
        va: 0xffffff800040e34a,
        expect: &[0x89, 0xc0, 0x81, 0xe2, 0xff, 0xff, 0x00, 0xff, 0x81, 0xca, 0x00, 0x00, 0x01, 0x00, 0x0f, 0x30],
        replace: &[0x89, 0xc0, 0xb8, 0x06, 0x01, 0x07, 0x00, 0xba, 0x06, 0x01, 0x07, 0x00, 0x0f, 0x30, 0x90, 0x90],
        why: "mtrr_update_action: fix PAT #2 (amd_vanilla)",
    },
];

// always applied: the stock macOS 27 x86_64 kernel runs its early memory reservations before
// virtual_space_start/end are set (they stay 0), so the first reserved range is mapped at address 0.
// the stub asks pmap_virtual_space() for the real range first, then continues into the original code.
// used only when no corecrypto image (xkc.bin) is staged
const NOKEXT_PATCHES: &[Patch] = &[Patch {
    va: 0xffff_ff80_00b0_3afd,
    expect: &[0x55, 0x48, 0x89, 0xe5],
    replace: &[0xc3, 0x90, 0x90, 0x90],
    why: "skip exe boothash salt check (no corecrypto image)",
}];

const EARLY_VM_PATCHES: &[Patch] = &[
    Patch {
        va: 0xffff_ff80_0040_2ea0,
        expect: &[0x55, 0x48, 0x89, 0xe5],
        replace: &[0xc3, 0x90, 0x90, 0x90],
        why: "skip commpage_update_mach_approximate_time (interrupt-time write that faults sporadically)",
    },
    Patch {
        va: 0xffff_ff80_0040_2ef0,
        expect: &[0x55, 0x48, 0x89, 0xe5],
        replace: &[0xc3, 0x90, 0x90, 0x90],
        why: "skip commpage_update_mach_continuous_time",
    },
    Patch {
        va: 0xffff_ff80_0026_7de0,
        expect: &[0x55, 0x48, 0x89, 0xe5],
        replace: &[0xc3, 0x90, 0x90, 0x90],
        why: "skip arcade_ast (kills user processes that never register with the arcade daemon)",
    },
    Patch {
        va: 0xffff_ff80_003d_6b20,
        expect: &[0x55, 0x48, 0x89, 0xe5],
        replace: &[0xc3, 0x90, 0x90, 0x90],
        why: "skip x86_64_protect_data_const (kexts that start late write to constant data)",
    },
    Patch {
        va: 0xffff_ff80_0082_6a90,
        expect: &[0x55, 0x48, 0x89, 0xe5],
        replace: &[0xc3, 0x90, 0x90, 0x90],
        why: "skip trust_cache_runtime_init (needs the 27 amfi interface, absent with Tahoe's amfi)",
    },
    Patch {
        va: 0xffffff8000c42700,
        expect: &[0u8; 37],
        replace: &[0x50, 0x48, 0x8d, 0x3d, 0xf8, 0x3e, 0x29, 0x00, 0x48, 0x8d, 0x35, 0xf9, 0x3e, 0x29, 0x00, 0xe8, 0x6c, 0x2e, 0x79, 0xff, 0x58, 0x48, 0xc7, 0x05, 0x68, 0x77, 0x58, 0x00, 0x00, 0x40, 0x00, 0x00, 0xe9, 0xe8, 0xd2, 0xeb, 0xff],
        why: "early vm stub",
    },
    Patch {
        va: 0xffffff8000affa02,
        expect: &[0x48, 0xc7, 0x05, 0x7b, 0xa4],
        replace: &[0xe9, 0xf9, 0x2c, 0x14, 0x00],
        why: "early vm entry",
    },
    Patch {
        va: 0xffff_ff80_00af_e504,
        expect: &[0x48, 0x89, 0xde, 0x4c, 0x89, 0xf2],
        replace: &[0x4c, 0x89, 0xf6, 0x48, 0x89, 0xda],
        why: "early kernel map range bounds",
    },
    Patch {
        va: 0xffffff8000c42800,
        expect: &[0u8; 197],
        replace: &[0x57, 0x56, 0x52, 0x51, 0x41, 0x50, 0x41, 0x51, 0x50, 0x41, 0x52, 0x41, 0x53, 0x48, 0x8d, 0x05, 0x1c, 0x00, 0x00, 0x00, 0xff, 0xd0, 0x41, 0x5b, 0x41, 0x5a, 0x58, 0x41, 0x59, 0x41, 0x58, 0x59, 0x5a, 0x5e, 0x5f, 0x48, 0xb8, 0xe0, 0xe6, 0x3d, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff, 0xe0, 0x00, 0x48, 0x89, 0xd0, 0x48, 0x29, 0xf0, 0x0f, 0x96, 0xc1, 0x48, 0x3d, 0x01, 0x00, 0x00, 0x10, 0x0f, 0x93, 0xc0, 0x08, 0xc8, 0x75, 0x7e, 0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x50, 0x48, 0x89, 0xd3, 0x49, 0x89, 0xf6, 0x49, 0x89, 0xff, 0x48, 0xb8, 0x30, 0xaa, 0x3d, 0x00, 0x80, 0xff, 0xff, 0xff, 0x4c, 0x8b, 0xa8, 0xd8, 0xd2, 0xc1, 0x00, 0x4c, 0x8b, 0xa0, 0xe0, 0xd2, 0xc1, 0x00, 0x49, 0x81, 0xe6, 0x00, 0xf0, 0xff, 0xff, 0xeb, 0x0e, 0x66, 0x90, 0x49, 0x81, 0xc6, 0x00, 0x10, 0x00, 0x00, 0x49, 0x39, 0xde, 0x73, 0x2a, 0x4c, 0x89, 0xff, 0x4c, 0x89, 0xf6, 0x48, 0xb8, 0x30, 0xaa, 0x3d, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff, 0xd0, 0x48, 0x3d, 0x00, 0x10, 0x00, 0x00, 0x72, 0xda, 0x48, 0xc1, 0xe8, 0x0c, 0x4c, 0x39, 0xe8, 0x77, 0xd1, 0x41, 0x80, 0x24, 0x04, 0xfe, 0xeb, 0xca, 0x48, 0x83, 0xc4, 0x08, 0x5b, 0x41, 0x5c, 0x41, 0x5d, 0x41, 0x5e, 0x41, 0x5f, 0x5d, 0xc3],
        why: "early pv repair code",
    },
    Patch {
        va: 0xffffff8000afbc36,
        expect: &[0xe8, 0xa5, 0x2a, 0x8e, 0xff],
        replace: &[0xe8, 0xc5, 0x6b, 0x14, 0x00],
        why: "early pv repair hook",
    },
    Patch {
        va: 0xffff_ff80_00ab_5f10,
        expect: &[0xb8, 0x02, 0x00, 0x00, 0x00],
        replace: &[0xb8, 0x01, 0x00, 0x00, 0x00],
        why: "standalone kernel counts as a static collection",
    },
    Patch {
        va: 0xffff_ff80_0020_0018,
        expect: &[0x01, 0x80, 0x21, 0x00],
        replace: &[0x01, 0x80, 0x21, 0x80],
        why: "kernel header says it is in a collection",
    },
];

struct Seg {
    pa: u64,
    vmsize: u64,
    off: u64,
    fsize: u64,
}

struct Kernel {
    segs: Vec<Seg>,
    entry_pa: u64,
    end_pa: u64,
}

fn rd32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn rd64(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}

fn parse(k: &[u8]) -> Result<Kernel, &'static str> {
    if k.len() < 32 || rd32(k, 0) != 0xfeed_facf {
        return Err("not a 64-bit mach-o");
    }
    if rd32(k, 4) != 0x0100_0007 {
        return Err("not an x86_64 kernel");
    }
    let ncmds = rd32(k, 16) as usize;
    let mut off = 32usize;
    let mut segs = Vec::new();
    let mut entry_va = 0u64;
    let mut end_pa = 0u64;
    for _ in 0..ncmds {
        let cmd = rd32(k, off);
        let size = rd32(k, off + 4) as usize;
        if cmd == 0x19 {
            let vmaddr = rd64(k, off + 24);
            let vmsize = rd64(k, off + 32);
            let fileoff = rd64(k, off + 40);
            let filesize = rd64(k, off + 48);
            if vmsize != 0 {
                if vmaddr < KBASE {
                    return Err("segment below the kernel base");
                }
                let pa = vmaddr - KBASE;
                end_pa = end_pa.max(pa + vmsize);
                segs.push(Seg { pa, vmsize, off: fileoff, fsize: filesize });
            }
        } else if cmd == 0x5 {
            entry_va = rd64(k, off + 16 + 16 * 8);
        }
        off += size;
    }
    if entry_va < KBASE {
        return Err("no entry point");
    }
    Ok(Kernel { segs, entry_pa: entry_va - KBASE, end_pa })
}

fn align_up(v: u64, a: u64) -> u64 {
    (v + a - 1) & !(a - 1)
}

fn node(props: &[(&str, &[u8])], children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(props.len() as u32).to_le_bytes());
    out.extend_from_slice(&(children.len() as u32).to_le_bytes());
    for (name, value) in props {
        let mut n = [0u8; 32];
        let nb = name.as_bytes();
        n[..nb.len()].copy_from_slice(nb);
        out.extend_from_slice(&n);
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    for c in children {
        out.extend_from_slice(c);
    }
    out
}

#[target_feature(enable = "rdrand")]
unsafe fn rdrand64() -> u64 {
    let mut v = 0u64;
    for _ in 0..16 {
        if core::arch::x86_64::_rdrand64_step(&mut v) == 1 {
            return v;
        }
    }
    core::arch::x86_64::_rdtsc()
}

fn random_seed() -> [u8; 64] {
    let mut out = [0u8; 64];
    let mut x = unsafe { core::arch::x86_64::_rdtsc() } | 1;
    for i in 0..8 {
        let r = unsafe { rdrand64() };
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out[i * 8..i * 8 + 8].copy_from_slice(&(r ^ x).to_le_bytes());
    }
    out
}

fn acpi_sum(b: &[u8]) -> u8 {
    0u8.wrapping_sub(b.iter().fold(0u8, |a, &x| a.wrapping_add(x)))
}

unsafe fn acpi_table(at: u64) -> &'static [u8] {
    let len = core::ptr::read_unaligned((at + 4) as *const u32) as usize;
    core::slice::from_raw_parts(at as *const u8, len.min(0x40_0000))
}

// b550/a520 firmware declares cpus only as ACPI0007 devices under \_SB.PLTF, macos needs Processor objects.
// same fix as dortania's ssdt-cpur: add one ssdt with a Processor per thread, through a new xsdt.
fn add_amd_cpu_ssdt(force: bool) {
    let Some(st) = uefi::table::system_table_raw() else { return };
    const ACPI20: [u8; 16] = [0x71, 0xe8, 0x68, 0x88, 0xf1, 0xe4, 0xd3, 0x11, 0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81];
    unsafe {
        let src = st.as_ptr() as *const u8;
        let n = core::ptr::read_unaligned(src.add(104) as *const u64) as usize;
        let cfg = core::ptr::read_unaligned(src.add(112) as *const u64) as *const u8;
        let mut rsdp = 0u64;
        for i in 0..n.min(64) {
            let e = cfg.add(i * 24);
            if core::slice::from_raw_parts(e, 16) == ACPI20 {
                rsdp = core::ptr::read_unaligned(e.add(16) as *const u64);
            }
        }
        if rsdp == 0 || *((rsdp + 15) as *const u8) < 2 {
            println!("acpi: no 2.0 root pointer, cpu table not added");
            return;
        }
        let xsdt = acpi_table(core::ptr::read_unaligned((rsdp + 24) as *const u64));
        let mut has_processor = false;
        let mut threads = 0usize;
        for e in xsdt[36..].chunks_exact(8) {
            let at = u64::from_le_bytes(e.try_into().unwrap());
            let t = acpi_table(at);
            let mut aml: &[u8] = &[];
            if &t[..4] == b"SSDT" {
                aml = t;
            } else if &t[..4] == b"FACP" && t.len() >= 148 {
                let x = u64::from_le_bytes(t[140..148].try_into().unwrap());
                let d = if x != 0 { x } else { u32::from_le_bytes(t[40..44].try_into().unwrap()) as u64 };
                aml = acpi_table(d);
            }
            if aml.len() <= 36 {
                continue;
            }
            let body = &aml[36..];
            has_processor |= body.windows(2).any(|w| w == [0x5b, 0x83]);
            let found = body.windows(8).filter(|w| *w == b"ACPI0007").count();
            if found > 0 && body.windows(4).any(|w| w == b"PLTF") && body.windows(4).any(|w| w == b"C000") {
                threads = found.min(32);
            }
        }
        if threads == 0 || (has_processor && !force) {
            println!("acpi: cpu table not needed (processor objects {}, pltf cpus {})", has_processor, threads);
            return;
        }
        let hex = b"0123456789ABCDEF";
        let mut scope: Vec<u8> = b"\\_SB_".to_vec();
        for i in 0..threads {
            scope.extend_from_slice(&[0x5b, 0x83, 27, b'P', b'R', b'0' + (i / 10) as u8, b'0' + (i % 10) as u8, i as u8, 0x10, 0x08, 0, 0, 6]);
            scope.extend_from_slice(&[0xa4, b'\\', 0x2f, 3]);
            scope.extend_from_slice(b"_SB_PLTFC0");
            scope.extend_from_slice(&[hex[i >> 4], hex[i & 15]]);
        }
        let plen = scope.len() + 2;
        let mut ssdt = vec![0u8; 36];
        ssdt[..4].copy_from_slice(b"SSDT");
        ssdt[8] = 2;
        ssdt[10..16].copy_from_slice(b"ANTHMA");
        ssdt[16..24].copy_from_slice(b"CPUR    ");
        ssdt[24] = 1;
        ssdt[28..32].copy_from_slice(b"ANTH");
        ssdt[32] = 1;
        ssdt.extend_from_slice(&[0x10, 0x40 | (plen & 15) as u8, (plen >> 4) as u8]);
        ssdt.extend_from_slice(&scope);
        let l = ssdt.len() as u32;
        ssdt[4..8].copy_from_slice(&l.to_le_bytes());
        ssdt[9] = acpi_sum(&ssdt);
        let ssdt_off = align_up((xsdt.len() + 8) as u64, 64) as usize;
        let total = ssdt_off + ssdt.len();
        let Ok(mem) = boot::allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::ACPI_RECLAIM, pages(total as u64)) else {
            println!("acpi: no memory for the cpu table");
            return;
        };
        let base = mem.as_ptr() as u64;
        let mut nx = xsdt.to_vec();
        nx.extend_from_slice(&(base + ssdt_off as u64).to_le_bytes());
        let l = nx.len() as u32;
        nx[4..8].copy_from_slice(&l.to_le_bytes());
        nx[9] = 0;
        nx[9] = acpi_sum(&nx);
        core::ptr::copy_nonoverlapping(nx.as_ptr(), base as *mut u8, nx.len());
        core::ptr::copy_nonoverlapping(ssdt.as_ptr(), (base as usize + ssdt_off) as *mut u8, ssdt.len());
        core::ptr::write_unaligned((rsdp + 24) as *mut u64, base);
        *((rsdp + 32) as *mut u8) = 0;
        let sum = acpi_sum(core::slice::from_raw_parts(rsdp as *const u8, 36));
        *((rsdp + 32) as *mut u8) = sum;
        let now = core::ptr::read_unaligned((rsdp + 24) as *const u64);
        println!("acpi: added cpu table for {} threads at {:#x}, root pointer updated {}", threads, base, now == base);
    }
}

fn device_tree() -> Vec<u8> {
    let memory_map = node(&[("name", b"memory-map\0")], &[]);
    let seed = random_seed();
    let chosen = node(&[("name", b"chosen\0"), ("random-seed", &seed)], &[memory_map]);
    let fsb = 100_000_000u64.to_le_bytes();
    // the kernel waits forever for the platform uuid resource, which on x86 it derives from a 16 byte
    // system-id on /efi/platform (a real mac takes it from the smbios system uuid)
    let (sa, sb) = unsafe { (rdrand64(), rdrand64()) };
    let mut sysid = [0u8; 16];
    sysid[..8].copy_from_slice(&sa.to_le_bytes());
    sysid[8..].copy_from_slice(&sb.to_le_bytes());
    let platform = node(&[("name", b"platform\0"), ("FSBFrequency", &fsb), ("system-id", &sysid)], &[]);
    // one node per firmware configuration table, named by its GUID: the acpi driver finds its root
    // table (and smbios) at /efi/configuration-table/<GUID> in the "table" property.
    let mut cfg_nodes: Vec<Vec<u8>> = Vec::new();
    if let Some(st) = uefi::table::system_table_raw() {
        unsafe {
            let src = st.as_ptr() as *const u8;
            let n = core::ptr::read_unaligned(src.add(104) as *const u64) as usize;
            let cfg = core::ptr::read_unaligned(src.add(112) as *const u64) as *const u8;
            for i in 0..n.min(64) {
                let e = cfg.add(i * 24);
                let mut g = [0u8; 16];
                core::ptr::copy_nonoverlapping(e, g.as_mut_ptr(), 16);
                let addr = core::ptr::read_unaligned(e.add(16) as *const u64);
                let d1 = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
                let d2 = u16::from_le_bytes([g[4], g[5]]);
                let d3 = u16::from_le_bytes([g[6], g[7]]);
                let mut name = alloc::format!(
                    "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                    d1, d2, d3, g[8], g[9], g[10], g[11], g[12], g[13], g[14], g[15]
                )
                .into_bytes();
                name.push(0);
                let tb = addr.to_le_bytes();
                cfg_nodes.push(node(&[("name", &name), ("guid", &g), ("table", &tb)], &[]));
            }
        }
    }
    let cfg_table = node(&[("name", b"configuration-table\0")], &cfg_nodes);
    let efi = node(&[("name", b"efi\0")], &[platform, cfg_table]);
    let options = node(&[("name", b"options\0"), ("platform-uuid", &sysid)], &[]);
    node(&[("name", b"device-tree\0"), ("compatible", b"ACPI\0"), ("model", b"Anathema1,1\0")], &[chosen, efi, options])
}

// a minimal MH_FILESET kernel collection header: one segment covering the kernel and one entry, the kernel itself.
// the kernel only needs this to take its "boot kernel collection" paths; there are no kexts in it yet.
// set to true to list the kernel itself as a collection entry (it gets counted twice by kalloc, so off for now)
fn std_entry() -> bool {
    true
}

fn fake_kc(info_addr: u64, info_size: u64) -> Vec<u8> {
    let mut h: Vec<u8> = Vec::new();
    let w32 = |h: &mut Vec<u8>, v: u32| h.extend_from_slice(&v.to_le_bytes());
    let w64 = |h: &mut Vec<u8>, v: u64| h.extend_from_slice(&v.to_le_bytes());
    w32(&mut h, 0xfeed_facf);
    w32(&mut h, 0x0100_0007);
    w32(&mut h, 3);
    w32(&mut h, 0xc);
    w32(&mut h, 4);
    w32(&mut h, 72 + 152 + 72 + 56);
    w32(&mut h, 0);
    w32(&mut h, 0);
    // LC_SEGMENT_64 __TEXT
    w32(&mut h, 0x19);
    w32(&mut h, 72);
    let mut name = [0u8; 16];
    name[..6].copy_from_slice(b"__TEXT");
    h.extend_from_slice(&name);
    w64(&mut h, KBASE + 0x20_0000);
    w64(&mut h, 0x105_2000);
    w64(&mut h, 0);
    w64(&mut h, 0);
    w32(&mut h, 5);
    w32(&mut h, 5);
    w32(&mut h, 0);
    w32(&mut h, 0);
    // LC_SEGMENT_64 __PRELINK_INFO with an empty __info section: the kernel looks it up without a null check
    w32(&mut h, 0x19);
    w32(&mut h, 72 + 80);
    let mut pn = [0u8; 16];
    pn[..14].copy_from_slice(b"__PRELINK_INFO");
    h.extend_from_slice(&pn);
    w64(&mut h, if info_size > 0 { info_addr } else { KBASE + 0x20_0000 + 0x105_2000 });
    w64(&mut h, (info_size + 0xfff) & !0xfff);
    w64(&mut h, 0);
    w64(&mut h, 0);
    w32(&mut h, 3);
    w32(&mut h, 3);
    w32(&mut h, 1);
    w32(&mut h, 0);
    let mut sn = [0u8; 16];
    sn[..6].copy_from_slice(b"__info");
    h.extend_from_slice(&sn);
    h.extend_from_slice(&pn);
    w64(&mut h, if info_size > 0 { info_addr } else { KBASE + 0x20_0000 + 0x105_2000 });
    w64(&mut h, info_size);
    w32(&mut h, 0);
    w32(&mut h, 0);
    w32(&mut h, 0);
    w32(&mut h, 0);
    w32(&mut h, 0);
    w32(&mut h, 0);
    w32(&mut h, 0);
    w32(&mut h, 0);
    // LC_SEGMENT_64 __LINKEDIT (empty): the kernel jettisons the collection's linkedit segment and
    // panics if the header has none
    w32(&mut h, 0x19);
    w32(&mut h, 72);
    let mut ln = [0u8; 16];
    ln[..10].copy_from_slice(b"__LINKEDIT");
    h.extend_from_slice(&ln);
    w64(&mut h, KBASE + 0x20_0000 + 0x105_2000);
    w64(&mut h, 0);
    w64(&mut h, 0);
    w64(&mut h, 0);
    w32(&mut h, 1);
    w32(&mut h, 1);
    w32(&mut h, 0);
    w32(&mut h, 0);
    // LC_FILESET_ENTRY com.apple.kernel
    w32(&mut h, 0x8000_0035);
    w32(&mut h, 56);
    w64(&mut h, KBASE + 0x20_0000);
    w64(&mut h, 0);
    w32(&mut h, 32);
    w32(&mut h, 0);
    let mut id = [0u8; 24];
    id[..16].copy_from_slice(b"com.apple.kernel");
    h.extend_from_slice(&id);
    h
}

// a block of kexts linked offline by tools/kcbuild.py. layout of xkc.bin: eight u64 (magic, base va,
// kext bytes, plist offset, plist length, corecrypto start va, corecrypto kmod_info va, blob length)
// then the blob. the block is copied to its address and declared to the kernel through the empty
// __PRELINK_TEXT segment of its own in-memory header (finalize marks such segments rwx).

// a stub EFI runtime services table: valid signature and checksum, every service returns
// EFI_NOT_FOUND. lets the nvram driver start (the kernel only publishes the platform uuid once it has
// registered) without the firmware's real runtime services, which can't be mapped yet.
unsafe fn install_runtime_stub(rt_va: u64) {
    let pa = (rt_va - KBASE) as *mut u8;
    core::ptr::write_bytes(pa, 0, 0x1000);
    let stub_va = rt_va + 0x100;
    let code: [u8; 11] = [0x48, 0xb8, 0x0e, 0, 0, 0, 0, 0, 0, 0x80, 0xc3];
    core::ptr::copy_nonoverlapping(code.as_ptr(), pa.add(0x100), code.len());
    let mut t = [0u8; 24 + 14 * 8];
    t[0..8].copy_from_slice(&0x5652_4553_544e_5552u64.to_le_bytes());
    t[8..12].copy_from_slice(&0x0002_0000u32.to_le_bytes());
    let tl = t.len() as u32;
    t[12..16].copy_from_slice(&tl.to_le_bytes());
    for i in 0..14 {
        t[24 + i * 8..32 + i * 8].copy_from_slice(&stub_va.to_le_bytes());
    }
    let c = crc32(&t);
    t[16..20].copy_from_slice(&c.to_le_bytes());
    core::ptr::copy_nonoverlapping(t.as_ptr(), pa, t.len());
}

// a stand-in for the amfi interface the kernel calls through (struct of function pointers at the
// kernel's `amfi` global): every entry is a stub returning 0, meaning no entitlement present and
// nothing denied. tahoe's amfi can't serve the 27 kernel's entitlement bridge, and nothing needs it to
// reach a root mount.
const AMFI_GLOBAL_PA: u64 = 0x117_d1d8;

unsafe fn install_amfi_standin(va: u64) {
    let pa = (va - KBASE) as *mut u8;
    core::ptr::write_bytes(pa, 0, 0x1000);
    let stub_va = va + 0x800;
    let code: [u8; 3] = [0x31, 0xc0, 0xc3];
    core::ptr::copy_nonoverlapping(code.as_ptr(), pa.add(0x800), code.len());
    for i in 0..(0x800 / 8) {
        core::ptr::copy_nonoverlapping(stub_va.to_le_bytes().as_ptr(), pa.add(i * 8), 8);
    }
    core::ptr::copy_nonoverlapping(va.to_le_bytes().as_ptr(), AMFI_GLOBAL_PA as *mut u8, 8);
}

static mut CP_STORE_VA: u64 = 0;
const LOG_PA: u64 = 0x400_0000;
static mut FB_BASE: u64 = 0;
static mut FB_STRIDE: u64 = 0;
static mut SBX_VA: u64 = 0;

unsafe fn install_kc(blob: &[u8]) {
    let base_va = rd64(blob, 8);
    let total = rd64(blob, 16);
    let blen = rd64(blob, 56) as usize;
    let pa = base_va - KBASE;
    core::ptr::copy(blob.as_ptr().add(64), pa as *mut u8, blen);
    let hdr = 0x20_0000u64 as *mut u8;
    let ncmds = *(hdr.add(16) as *const u32);
    let mut o = 32usize;
    for _ in 0..ncmds {
        let cmd = *(hdr.add(o) as *const u32);
        let sz = *(hdr.add(o + 4) as *const u32) as usize;
        if cmd == 0x19 {
            let nm = core::slice::from_raw_parts(hdr.add(o + 8), 15);
            if nm == b"__PRELINK_TEXT\0" {
                *(hdr.add(o + 24) as *mut u64) = base_va;
                *(hdr.add(o + 32) as *mut u64) = ((blen as u64 + 0xfff) & !0xfff).max(total);
            }
        }
        o += sz;
    }
    install_runtime_stub(base_va + blen as u64 - 0x2000);
    CP_STORE_VA = base_va + blen as u64 - 0x2000 + 0x800;
    // the last 8 KB of the plist reserve are unused and rwx: room for the stand-in sandbox policy's tables
    SBX_VA = base_va + rd64(blob, 24) + 0x80000 - 0x2000;
    install_amfi_standin(base_va + blen as u64 - 0x1000);
    com1("anathema-xboot: kext block installed\n");
}

// the 27 intel kernel has no code that creates the commpage maps: vm_commpage_init and
// vm_commpage_text_init only call the populate routines. xcp.bin holds two small replacements
// (built from tools/commpage_stub.c) that create the four maps and then call those routines.
// the first stub is at offset 0 and the second at 0x100.
const CP_PA: u64 = 0xc4_2a00;
const CP_INIT_PA: u64 = 0x39_c290;
const CP_TEXT_INIT_PA: u64 = 0x39_c1f0;

const CP2_PA: u64 = 0xc4_2c40;
const VM_MAP_EXEC_PA: u64 = 0x35_0e80;

// maps the commpage into every exec'd user task: the 27 intel kernel has no code that does it
unsafe fn install_commpage_enter(blob: &[u8]) {
    if CP_STORE_VA == 0 || blob.len() < 0x40 || blob.len() > 0x3c0 {
        com1("anathema-xboot: commpage enter stub not installed\n");
        return;
    }
    let mut buf = [0u8; 0x3c0];
    let n = blob.len();
    buf[..n].copy_from_slice(blob);
    let code = &mut buf[..n];
    let ph = 0x1122_3344_5566_7788u64.to_le_bytes();
    let mut i = 0;
    while i + 8 <= code.len() {
        if code[i..i + 8] == ph {
            code[i..i + 8].copy_from_slice(&CP_STORE_VA.to_le_bytes());
            i += 8;
        } else {
            i += 1;
        }
    }
    let site = VM_MAP_EXEC_PA as *mut u8;
    let want = [0x55u8, 0x48, 0x89, 0xe5, 0x41, 0x57];
    for (k, b) in want.iter().enumerate() {
        if *site.add(k) != *b {
            com1("anathema-xboot: vm_map_exec bytes differ, commpage enter not installed\n");
            return;
        }
    }
    core::ptr::copy_nonoverlapping(code.as_ptr(), CP2_PA as *mut u8, code.len());
    let rel = (CP2_PA as i64 - (VM_MAP_EXEC_PA as i64 + 5)) as i32;
    *site = 0xe9;
    core::ptr::copy_nonoverlapping(rel.to_le_bytes().as_ptr(), site.add(1), 4);
    *site.add(5) = 0x90;
    com1("anathema-xboot: commpage enter hook installed\n");
    let site2 = 0x34_33a0u64 as *mut u8;
    let want2 = [0x55u8, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x48, 0x83, 0xec, 0x38, 0x4c, 0x8b, 0x57, 0x18];
    for (k, b) in want2.iter().enumerate() {
        if *site2.add(k) != *b {
            com1("anathema-xboot: fixed locator bytes differ, hook not installed\n");
            return;
        }
    }
    let rel2 = ((CP2_PA + 0x40) as i64 - (0x34_33a0i64 + 5)) as i32;
    *site2 = 0xe9;
    core::ptr::copy_nonoverlapping(rel2.to_le_bytes().as_ptr(), site2.add(1), 4);
    *site2.add(5) = 0x90;
    com1("anathema-xboot: commpage fixed locator hook installed\n");
}

// starts kexts that have no matching personality (tmpfs, the fs compression types) by calling the kernel's own
// OSKext::loadKextWithIdentifier right before launchd starts. code lives in the spare part of the kext block.
const LOAD_INIT_PA: u64 = 0x7e_0bc0;

unsafe fn install_loadkexts(blob: &[u8]) {
    if CP_STORE_VA == 0 || blob.len() < 0x40 || blob.len() > 0x700 {
        com1("anathema-xboot: loadkexts stub not installed\n");
        return;
    }
    let site = LOAD_INIT_PA as *mut u8;
    let want = [0x55u8, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56];
    for (k, b) in want.iter().enumerate() {
        if *site.add(k) != *b {
            com1("anathema-xboot: load_init_program bytes differ, loadkexts not installed\n");
            return;
        }
    }
    let dst_va = CP_STORE_VA - 0x800 + 0x900;
    // the stand-in sandbox policy's data (conf, ops table, handle) lives in the amfi page, past the amfi stand-in
    let sbx_va = SBX_VA;
    core::ptr::write_bytes(sbx_va.wrapping_sub(KBASE) as *mut u8, 0, 0x2000);
    let mut buf = [0u8; 0x700];
    let n = blob.len();
    buf[..n].copy_from_slice(blob);
    let ph = 0xaabb_ccdd_eeff_1122u64.to_le_bytes();
    let mut i = 0;
    while i + 8 <= n {
        if buf[i..i + 8] == ph {
            buf[i..i + 8].copy_from_slice(&sbx_va.to_le_bytes());
            i += 8;
        } else {
            i += 1;
        }
    }
    core::ptr::copy_nonoverlapping(buf.as_ptr(), dst_va.wrapping_sub(KBASE) as *mut u8, n);
    let rel = (dst_va as i64 - (KBASE as i64 + LOAD_INIT_PA as i64 + 5)) as i32;
    *site = 0xe9;
    core::ptr::copy_nonoverlapping(rel.to_le_bytes().as_ptr(), site.add(1), 4);
    *site.add(5) = 0x90;
    com1("anathema-xboot: loadkexts hook installed\n");
}

// early-boot progress markers: tiny kernel hooks paint colored bands on the framebuffer, so a photo shows how far
// the kernel got on hardware without a serial port (see tools/mkmark.py for the blob format)
unsafe fn install_markers(blob: &[u8]) {
    if FB_BASE == 0 || blob.len() < 8 || rd32(blob, 0) != 0x4b52_414d {
        return;
    }
    let count = rd32(blob, 4) as usize;
    let mut o = 8usize;
    let stride = FB_STRIDE;
    for _ in 0..count {
        let site = rd64(blob, o);
        let slen = blob[o + 8] as usize;
        let mut expect = [0u8; 8];
        expect.copy_from_slice(&blob[o + 9..o + 17]);
        let cave = rd64(blob, o + 17);
        let clen = u16::from_le_bytes([blob[o + 25], blob[o + 26]]) as usize;
        o += 27;
        let mut buf = [0u8; 160];
        if clen > buf.len() {
            return;
        }
        buf[..clen].copy_from_slice(&blob[o..o + clen]);
        o += clen;
        let mut i = 0;
        while i + 4 <= clen {
            let v4 = u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
            if v4 == 0xdead_0001 {
                buf[i..i + 4].copy_from_slice(&((6 * stride) as u32).to_le_bytes());
                i += 4;
                continue;
            }
            if v4 & 0xffff_ff00 == 0x3322_1100 {
                let k = (v4 & 0xff) as u64;
                let v = (FB_BASE + (k * 10 + 4) * stride * 4) as u32;
                buf[i..i + 4].copy_from_slice(&v.to_le_bytes());
                i += 4;
                continue;
            }
            if i + 8 <= clen {
                let mut e = [0u8; 8];
                e.copy_from_slice(&buf[i..i + 8]);
                let v8 = u64::from_le_bytes(e);
                if v8 & 0xffff_ffff_ffff_0000 == 0x5566_7788_99aa_0000 {
                    let k = (v8 >> 8) & 0xff;
                    let v = KBASE + LOG_PA + k;
                    buf[i..i + 8].copy_from_slice(&v.to_le_bytes());
                    i += 8;
                    continue;
                }
                if v8 & 0xffff_ffff_ffff_0000 == 0x1122_3344_5566_0000 {
                    let k = (v8 >> 8) & 0xff;
                    let mode = v8 & 0xff;
                    let mut v = FB_BASE + (k * 10 + 4) * stride * 4;
                    if mode == 1 {
                        v += KBASE;
                    }
                    buf[i..i + 8].copy_from_slice(&v.to_le_bytes());
                    i += 8;
                    continue;
                }
            }
            i += 1;
        }
        let sp = site as *mut u8;
        let mut same = true;
        for k in 0..slen.min(8) {
            if *sp.add(k) != expect[k] {
                same = false;
            }
        }
        if !same {
            com1(&alloc::format!("xmark: skipped site {:#x} (bytes differ)\n", site));
            continue;
        }
        core::ptr::copy_nonoverlapping(buf.as_ptr(), cave as *mut u8, clen);
        let rel = (cave as i64 - (site as i64 + 5)) as i32;
        *sp = 0xe9;
        core::ptr::copy_nonoverlapping(rel.to_le_bytes().as_ptr(), sp.add(1), 4);
        for k in 5..slen {
            *sp.add(k) = 0x90;
        }
    }
    com1("anathema-xboot: early markers installed\n");
}

unsafe fn paint_band(row: u64, color: u32) {
    if FB_BASE == 0 {
        return;
    }
    let p = (FB_BASE + row * FB_STRIDE * 4) as *mut u32;
    for i in 0..(6 * FB_STRIDE) {
        *p.add(i as usize) = color;
    }
}

unsafe fn install_commpage(blob: &[u8]) {
    let n = blob.len().min(0x400);
    core::ptr::copy_nonoverlapping(blob.as_ptr(), CP_PA as *mut u8, n);
    let sites = [(CP_INIT_PA, CP_PA), (CP_TEXT_INIT_PA, CP_PA + 0x100)];
    for (site, target) in sites {
        let hook = site as *mut u8;
        let want = [0x55u8, 0x48, 0x89, 0xe5, 0x83];
        let mut same = true;
        for (i, b) in want.iter().enumerate() {
            if *hook.add(i) != *b {
                same = false;
            }
        }
        if same {
            let rel = (target as i64 - (site as i64 + 5)) as i32;
            *hook = 0xe9;
            core::ptr::copy_nonoverlapping(rel.to_le_bytes().as_ptr(), hook.add(1), 4);
            com1("anathema-xboot: commpage map stub installed\n");
        } else {
            com1("anathema-xboot: commpage site bytes differ, not patched\n");
        }
    }
}

// amd cpus: after the vendor check is skipped the kernel still wants family 6 and a known intel model
// to pick a cpu family. overwrite its decoded family and model with a haswell (family 6, model 0x3c)
// right where it compares them, from a small cave in the kernel text.
const AMD_CAVE_PA: u64 = 0xc4_2c00;
const CPUID_FAMILY_PA: u64 = 0x10b_e14c;
const CPUID_MODEL_PA: u64 = 0x10b_e14d;
const CPUID_STEPPING_PA: u64 = 0x10b_e150;
const AMD_SITE_PA: u64 = 0x3e_58a5;
const AMD_BACK_PA: u64 = 0x3e_58ae;

unsafe fn install_amd_cpuid_spoof() {
    let site = AMD_SITE_PA as *mut u8;
    let want = [0x80u8, 0x3d];
    if *site != want[0] || *site.add(1) != want[1] || *site.add(6) != 0x06 {
        com1("anathema-xboot: amd cpuid site bytes differ, not patched\n");
        return;
    }
    let cave = AMD_CAVE_PA as *mut u8;
    let r1 = (CPUID_FAMILY_PA as i64 - (AMD_CAVE_PA as i64 + 7)) as i32;
    let r2 = (CPUID_MODEL_PA as i64 - (AMD_CAVE_PA as i64 + 14)) as i32;
    let r3 = (CPUID_STEPPING_PA as i64 - (AMD_CAVE_PA as i64 + 21)) as i32;
    let r4 = (AMD_BACK_PA as i64 - (AMD_CAVE_PA as i64 + 26)) as i32;
    let mut code = [0u8; 26];
    code[0] = 0xc6;
    code[1] = 0x05;
    code[2..6].copy_from_slice(&r1.to_le_bytes());
    code[6] = 0x06;
    code[7] = 0xc6;
    code[8] = 0x05;
    code[9..13].copy_from_slice(&r2.to_le_bytes());
    code[13] = 0x3c;
    code[14] = 0xc6;
    code[15] = 0x05;
    code[16..20].copy_from_slice(&r3.to_le_bytes());
    code[20] = 0x03;
    code[21] = 0xe9;
    code[22..26].copy_from_slice(&r4.to_le_bytes());
    core::ptr::copy_nonoverlapping(code.as_ptr(), cave, code.len());
    let to_cave = (AMD_CAVE_PA as i64 - (AMD_SITE_PA as i64 + 5)) as i32;
    *site = 0xe9;
    core::ptr::copy_nonoverlapping(to_cave.to_le_bytes().as_ptr(), site.add(1), 4);
    *site.add(5) = 0x90;
    *site.add(6) = 0x90;
    com1("anathema-xboot: amd cpuid spoofed to haswell\n");
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xedb8_8320 } else { c >> 1 };
        }
    }
    !c
}

// a copy of the firmware system table with no runtime services, so the kernel only reads the acpi table list
fn fake_system_table(at: u64, rt_va: u64) -> Vec<u8> {
    let mut out = vec![0u8; 0x2000];
    if let Some(st) = uefi::table::system_table_raw() {
        let src = st.as_ptr() as *const u8;
        unsafe {
            let hs = core::ptr::read_unaligned(src.add(12) as *const u32) as usize;
            core::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), hs.min(0x100));
            let n = core::ptr::read_unaligned(src.add(104) as *const u64) as usize;
            let cfg = core::ptr::read_unaligned(src.add(112) as *const u64) as *const u8;
            let bytes = (n * 24).min(0x800);
            core::ptr::copy_nonoverlapping(cfg, out.as_mut_ptr().add(0x800), bytes);
            out[88..96].copy_from_slice(&rt_va.to_le_bytes());
            out[104..112].copy_from_slice(&((bytes / 24) as u64).to_le_bytes());
            out[112..120].copy_from_slice(&(at + 0x800).to_le_bytes());
            out[16..20].copy_from_slice(&0u32.to_le_bytes());
            let c = crc32(&out[..hs]);
            out[16..20].copy_from_slice(&c.to_le_bytes());
        }
    }
    out
}

fn com1_byte(b: u8) {
    unsafe {
        loop {
            let status: u8;
            asm!("in al, dx", out("al") status, in("dx") 0x3fdu16, options(nomem, nostack));
            if status & 0x20 != 0 {
                break;
            }
        }
        asm!("out dx, al", in("dx") 0x3f8u16, in("al") b, options(nomem, nostack));
    }
}

fn com1(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            com1_byte(b'\r');
        }
        com1_byte(b);
    }
}

fn pages(bytes: u64) -> usize {
    (align_up(bytes, 4096) / 4096) as usize
}

core::arch::global_asm!(
    ".global anx_tramp_start",
    ".global anx_tramp_end",
    "anx_tramp_start:",
    "cli",
    "mov cr3, rdx",
    "lgdt [rcx]",
    "lea rax, [rip + 2f]",
    "push 0x08",
    "push rax",
    "retfq",
    ".code32",
    "2:",
    "mov ax, 0x10",
    "mov ds, ax",
    "mov es, ax",
    "mov ss, ax",
    "mov fs, ax",
    "mov gs, ax",
    "mov eax, cr0",
    "and eax, 0x7fffffff",
    "mov cr0, eax",
    "mov ecx, 0xC0000080",
    "rdmsr",
    "and eax, 0xfffffeff",
    "wrmsr",
    "mov eax, cr4",
    "and eax, 0xffffffdf",
    "mov cr4, eax",
    "mov eax, edi",
    "jmp esi",
    ".code64",
    "anx_tramp_end:",
);

unsafe extern "C" {
    static anx_tramp_start: u8;
    static anx_tramp_end: u8;
}

static mut TRAMP: *mut u8 = core::ptr::null_mut();

unsafe fn handoff(pml4: u64, gdtr: u64, entry: u32, bootargs: u32) -> ! {
    let f: extern "sysv64" fn(u64, u64, u64, u64) -> ! = core::mem::transmute(TRAMP);
    f(bootargs as u64, entry as u64, pml4, gdtr)
}

fn fail(msg: &str) -> Status {
    println!("anathema-xboot: {}", msg);
    com1("anathema-xboot: ");
    com1(msg);
    com1("\n");
    boot::stall(10_000_000);
    Status::LOAD_ERROR
}

#[entry]
fn main() -> Status {
    let _ = boot::set_watchdog_timer(0, 0x10000, None);
    println!("anathema-xboot: native x86_64 macOS kernel loader");
    let mut prev_trail = String::new();
    unsafe {
        let mut v: u8;
        asm!("out dx, al", in("dx") 0x70u16, in("al") 1u8, options(nomem, nostack));
        asm!("in al, dx", out("al") v, in("dx") 0x71u16, options(nomem, nostack));
        asm!("out dx, al", in("dx") 0x70u16, in("al") 1u8, options(nomem, nostack));
        asm!("out dx, al", in("dx") 0x71u16, in("al") 0u8, options(nomem, nostack));
        if v == 0 {
            println!("previous boot trail: none");
        } else {
            println!("previous boot trail: last marker {}", v);
            prev_trail = alloc::format!("last marker {}", v);
        }
    }
    com1("anathema-xboot: start\n");

    let fs_proto = match boot::get_image_file_system(boot::image_handle()) {
        Ok(p) => p,
        Err(_) => return fail("cannot open the boot file system"),
    };
    let mut fs = FileSystem::new(fs_proto);
    let kernel_file: Vec<u8> = match fs.read(cstr16!("\\anathema\\xkernel")) {
        Ok(d) => d,
        Err(_) => return fail("cannot read \\anathema\\xkernel"),
    };
    let args: String = match fs.read(cstr16!("\\anathema\\xargs.txt")) {
        Ok(d) => String::from_utf8_lossy(&d).trim().into(),
        Err(_) => String::from(DEFAULT_ARGS),
    };
    let want_tsc_patch = fs.read(cstr16!("\\anathema\\xpatch-tsc")).is_ok();
    // "AuthenticAMD": the intel kernel needs its cpu identification patched on amd processors
    let is_amd = {
        let r = core::arch::x86_64::__cpuid(0);
        r.ebx == 0x6874_7541 && r.edx == 0x6974_6e65 && r.ecx == 0x444d_4163
    };
    let want_amd_patch = is_amd
        || fs.read(cstr16!("\\anathema\\xpatch-amd")).is_ok()
        || fs.read(cstr16!("\\anathema\\xpatch-spoof")).is_ok();
    if is_amd {
        println!("amd cpu detected: applying the amd patches");
    }
    // tsc speed in units of the 100 MHz bus, measured against the firmware timer: the kernel's tsc_init
    // reads it from MSR_PLATFORM_INFO, which qemu leaves at zero and amd cpus don't have
    let tsc_ratio: u32 = if want_tsc_patch || want_amd_patch || true {
        let t0 = unsafe { core::arch::x86_64::_rdtsc() };
        boot::stall(100_000);
        let t1 = unsafe { core::arch::x86_64::_rdtsc() };
        let hz = (t1 - t0) * 10;
        let r = ((hz + 50_000_000) / 100_000_000) as u32;
        println!("tsc: about {} MHz, ratio {}", hz / 1_000_000, r);
        r.clamp(8, 80)
    } else {
        0
    };
    let mut kext_blob: Vec<u8> = fs.read(cstr16!("\\anathema\\xkc.bin")).unwrap_or_default();
    if want_amd_patch {
        // iopcifamily probeBusGated: never turn on pcie 10-bit tags, amd_vanilla needs this on b550/x570
        const SIG: [u8; 12] = [0x0f, 0xba, 0xe3, 0x11, 0x73, 0x4a, 0x48, 0x8b, 0x41, 0x78, 0x48, 0x8b];
        let hits: Vec<usize> = kext_blob.windows(SIG.len()).enumerate().filter(|(_, w)| *w == SIG).map(|(i, _)| i).collect();
        if hits.len() == 1 {
            kext_blob[hits[0] + 4] = 0xeb;
            println!("amd: pci 10-bit tags disabled");
        } else {
            println!("amd: pci 10-bit tag site found {} times, not patched", hits.len());
        }
    }
    let have_kext = kext_blob.len() > 32 && rd64(&kext_blob, 0) == 0x434b_5841_4e41;
    let cp_blob: Vec<u8> = fs.read(cstr16!("\\anathema\\xcp.bin")).unwrap_or_default();
    let cp2_blob: Vec<u8> = fs.read(cstr16!("\\anathema\\xcp2.bin")).unwrap_or_default();
    let cp3_blob: Vec<u8> = fs.read(cstr16!("\\anathema\\xcp3.bin")).unwrap_or_default();
    let mark_blob: Vec<u8> = fs.read(cstr16!("\\anathema\\xmark.bin")).unwrap_or_default();
    let kern = match parse(&kernel_file) {
        Ok(k) => k,
        Err(e) => return fail(e),
    };
    println!("kernel: {} segments, entry {:#x}, image end {:#x}", kern.segs.len(), kern.entry_pa, kern.end_pa);

    let base = align_up(kern.end_pa, 4096);
    if want_amd_patch {
        add_amd_cpu_ssdt(fs.read(cstr16!("\\anathema\\xacpiforce")).is_ok());
    }
    let dt = device_tree();
    let boot_args_pa = base;
    let dt_pa = base + 0x1000;
    let mmap_pa = align_up(dt_pa + dt.len() as u64, 0x1000);
    let mmap_cap = 0x10000u64;
    let kc_pa = mmap_pa + mmap_cap;
    let (kc_plist_va, kc_plist_len) = if have_kext {
        (rd64(&kext_blob, 8) + rd64(&kext_blob, 24), rd64(&kext_blob, 32))
    } else {
        (0, 0)
    };
    let kc = fake_kc(kc_plist_va, kc_plist_len);
    let est_pa = kc_pa + 0x1000;
    let rt_va = if have_kext { rd64(&kext_blob, 8) + rd64(&kext_blob, 56) - 0x2000 } else { 0 };
    let est = fake_system_table(est_pa, rt_va);
    let final_end = kc_pa + 0x3000;
    let ksize_end = if have_kext { final_end.max(rd64(&kext_blob, 8) - KBASE + rd64(&kext_blob, 56)) } else { final_end };
    let kaddr = 0x100000u64;
    println!("boot data: args {:#x}, tree {:#x} ({} bytes), map {:#x}, end {:#x}", boot_args_pa, dt_pa, dt.len(), mmap_pa, final_end);

    let src_lo = kernel_file.as_ptr() as u64;
    let src_hi = src_lo + kernel_file.len() as u64;
    if src_lo < ksize_end && src_hi > kaddr {
        return fail("the kernel file buffer overlaps the load area");
    }
    if have_kext {
        let k_lo = kext_blob.as_ptr() as u64;
        let k_hi = k_lo + kext_blob.len() as u64;
        if k_lo < ksize_end && k_hi > kaddr {
            return fail("the kext buffer overlaps the load area");
        }
    }

    // identity map of the first 4 GB with 2 MB pages, plus a small GDT, both below 4 GB
    let table_ptr = match boot::allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, 7) {
        Ok(p) => p.as_ptr(),
        Err(_) => return fail("cannot allocate page tables"),
    };
    let tables = table_ptr as u64;
    unsafe {
        let start = core::ptr::addr_of!(anx_tramp_start) as usize;
        let end = core::ptr::addr_of!(anx_tramp_end) as usize;
        match boot::allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_CODE, 1) {
            Ok(pg) => {
                TRAMP = pg.as_ptr();
                core::ptr::copy_nonoverlapping(start as *const u8, TRAMP, end - start);
            }
            Err(_) => return fail("no low page for the handoff code"),
        }
        println!("image code at {:#x}, handoff code at {:#x}", start, TRAMP as usize);
    }
    if tables < ksize_end {
        return fail("page tables landed inside the load area");
    }
    unsafe {
        core::ptr::write_bytes(table_ptr, 0, 7 * 4096);
        let pml4 = table_ptr as *mut u64;
        let pdpt = table_ptr.add(4096) as *mut u64;
        *pml4 = (tables + 4096) | 3;
        for g in 0..4u64 {
            *pdpt.add(g as usize) = (tables + 8192 + g * 4096) | 3;
            let pd = table_ptr.add(8192 + g as usize * 4096) as *mut u64;
            for i in 0..512u64 {
                *pd.add(i as usize) = (g * 0x4000_0000 + i * 0x20_0000) | 0x83;
            }
        }
        let gdt = table_ptr.add(6 * 4096) as *mut u64;
        *gdt.add(0) = 0;
        *gdt.add(1) = 0x00cf_9a00_0000_ffff;
        *gdt.add(2) = 0x00cf_9200_0000_ffff;
        let gdtr = table_ptr.add(6 * 4096 + 0x40);
        *(gdtr as *mut u16) = 23;
        *(gdtr.add(2) as *mut u64) = tables + 6 * 4096;
    }
    let pml4_pa = tables;
    let gdtr_pa = tables + 6 * 4096 + 0x40;

    // boot_args staged in memory, patched with the memory map after boot services end
    let mut ba = vec![0u8; 4096];
    ba[0..2].copy_from_slice(&1u16.to_le_bytes());
    ba[2..4].copy_from_slice(&2u16.to_le_bytes());
    ba[4] = 64;
    let cl = args.as_bytes();
    let n = cl.len().min(1023);
    ba[8..8 + n].copy_from_slice(&cl[..n]);
    ba[1072..1076].copy_from_slice(&(dt_pa as u32).to_le_bytes());
    ba[1076..1080].copy_from_slice(&(dt.len() as u32).to_le_bytes());
    ba[1080..1084].copy_from_slice(&(kaddr as u32).to_le_bytes());
    ba[1084..1088].copy_from_slice(&((ksize_end - kaddr) as u32).to_le_bytes());
    ba[1152..1160].copy_from_slice(&100_000_000u64.to_le_bytes());
    ba[1256..1264].copy_from_slice(&(KBASE + kc_pa).to_le_bytes());
    ba[1104..1108].copy_from_slice(&((est_pa) as u32).to_le_bytes());

    let mut fb_note = String::from("framebuffer: no graphics output protocol found");
    // uefi framebuffer -> boot_args.Video so the kernel's console draws on the real screen
    if let Ok(h) = boot::get_handle_for_protocol::<uefi::proto::console::gop::GraphicsOutput>() {
        let params = boot::OpenProtocolParams { handle: h, agent: boot::image_handle(), controller: None };
        if let Ok(mut gop) = unsafe { boot::open_protocol::<uefi::proto::console::gop::GraphicsOutput>(params, boot::OpenProtocolAttributes::GetProtocol) } {
            let info = gop.current_mode_info();
            let (w, hgt) = info.resolution();
            let stride = info.stride();
            let fb = gop.frame_buffer().as_mut_ptr() as u64;
            if fb != 0 && fb < 0x1_0000_0000 {
                ba[1048..1052].copy_from_slice(&(fb as u32).to_le_bytes());
                let disp: u32 = if args.split_whitespace().any(|a| a == "-v") { 2 } else { 1 };
                ba[1052..1056].copy_from_slice(&disp.to_le_bytes());
                ba[1056..1060].copy_from_slice(&((stride * 4) as u32).to_le_bytes());
                ba[1060..1064].copy_from_slice(&(w as u32).to_le_bytes());
                ba[1064..1068].copy_from_slice(&(hgt as u32).to_le_bytes());
                ba[1068..1072].copy_from_slice(&32u32.to_le_bytes());
                unsafe {
                    FB_BASE = fb;
                    FB_STRIDE = stride as u64;
                }
                println!("framebuffer: {}x{} stride {} at {:#x}", w, hgt, stride, fb);
                fb_note = alloc::format!("framebuffer: {}x{} stride {} at {:#x} (passed to kernel)", w, hgt, stride, fb);
            } else {
                println!("framebuffer above 4 GB or missing, not passed ({:#x})", fb);
                fb_note = alloc::format!("framebuffer: {}x{} at {:#x} is above 4 GB or missing, NOT passed to kernel", w, hgt, fb);
            }
        }
    }
    if !args.is_empty() {
        println!("command line: {}", args);
    }
    let mut hw = String::new();
    unsafe {
        let rdm = |msr: u32| -> u64 {
            let lo: u32;
            let hi: u32;
            asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack));
            ((hi as u64) << 32) | lo as u64
        };
        let cr0: u64;
        let cr3: u64;
        let cr4: u64;
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack));
        asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack));
        hw.push_str(&alloc::format!("cr0 {:#x} cr3 {:#x} cr4 {:#x}\n", cr0, cr3, cr4));
        // the last three are amd msrs (tsc_aux also needs rdtscp): reading them on intel faults
        let msrs = [("APIC_BASE", 0x1bu32), ("EFER", 0xc000_0080), ("PAT", 0x277), ("MTRRdefType", 0x2ff), ("SYSCFG", 0xc001_0010), ("HWCR", 0xc001_0015), ("TSC_AUX", 0xc000_0103)];
        for &(n, m) in &msrs[..if is_amd { msrs.len() } else { 4 }] {
            hw.push_str(&alloc::format!("msr {} = {:#x}\n", n, rdm(m)));
        }
        for (l, s) in [(0u32, 0u32), (1, 0), (7, 0), (0xb, 0), (0xb, 1), (0x8000_0001, 0), (0x8000_0008, 0), (0x8000_001e, 0)] {
            let r = core::arch::x86_64::__cpuid_count(l, s);
            hw.push_str(&alloc::format!("cpuid {:#x}.{} = {:#x} {:#x} {:#x} {:#x}\n", l, s, r.eax, r.ebx, r.ecx, r.edx));
        }
    }
    if let Ok(mm) = boot::memory_map(MemoryType::LOADER_DATA) {
        use uefi::mem::memory_map::MemoryMap;
        hw.push_str(&alloc::format!("memory map entries: {}\n", mm.entries().count()));
        for e in mm.entries() {
            hw.push_str(&alloc::format!("  type {:?} phys {:#x} pages {:#x} attr {:#x}\n", e.ty, e.phys_start, e.page_count, e.att.bits()));
        }
    }
    let blog = alloc::format!(
        "anathema-xboot boot log\nprevious boot trail: {}\ncommand line: {}\n{}\nkernel entry {:#x}\n{}leaving boot services next\n",
        prev_trail, args, fb_note, kern.entry_pa, hw
    );
    let _ = fs.write(cstr16!("\\anathema\\boot.log"), blog.as_bytes());
    println!("leaving boot services (pausing 4 s)");
    boot::stall(4_000_000);
    com1("anathema-xboot: exiting boot services\n");

    let mmap = unsafe { boot::exit_boot_services(Some(MemoryType::LOADER_DATA)) };
    let raw = mmap.buffer();
    let meta = mmap.meta();
    let mut phys: u64 = 0;
    for d in mmap.entries() {
        let t = d.ty.0;
        if (1..=7).contains(&t) || t == 9 {
            phys += d.page_count * 4096;
        }
    }
    let copy_len = raw.len().min(mmap_cap as usize);

    unsafe {
        for s in kern.segs.iter() {
            let dst = s.pa as *mut u8;
            let take = s.fsize.min(s.vmsize) as usize;
            core::ptr::copy_nonoverlapping(kernel_file.as_ptr().add(s.off as usize), dst, take);
            if s.vmsize as usize > take {
                core::ptr::write_bytes(dst.add(take), 0, s.vmsize as usize - take);
            }
        }
        if have_kext {
            install_kc(&kext_blob);
        }
        if want_amd_patch {
            install_amd_cpuid_spoof();
        }
        if tsc_ratio != 0 {
            let site = 0x40_198du64 as *mut u8;
            let want = [0x48u8, 0x8b, 0x0d];
            if *site == want[0] && *site.add(1) == want[1] && *site.add(2) == want[2] {
                *site = 0xb9;
                core::ptr::copy_nonoverlapping(tsc_ratio.to_le_bytes().as_ptr(), site.add(1), 4);
                *site.add(5) = 0x90;
                *site.add(6) = 0x90;
                com1("anathema-xboot: tsc ratio patched\n");
            } else {
                com1("anathema-xboot: tsc ratio site differs, not patched\n");
            }
        }
        // gIOPlatformUUIDAndSerialDone: on intel the platform uuid is only published once the nvram
        // driver registers, and the security-extension step waits forever for it. until nvram has
        // runtime services behind it, mark it done so the kernel moves on.
        *((0x10_526c0u64) as *mut u8) = 1;
        if cp_blob.len() >= 0x100 {
            install_commpage(&cp_blob);
        }
        if !cp2_blob.is_empty() {
            install_commpage_enter(&cp2_blob);
        }
        if !cp3_blob.is_empty() {
            install_loadkexts(&cp3_blob);
        }
        if !mark_blob.is_empty() {
            install_markers(&mark_blob);
        }
        let empty: &[Patch] = &[];
        let lists: [&[Patch]; 4] = [
            EARLY_VM_PATCHES,
            empty,
            if have_kext { empty } else { NOKEXT_PATCHES },
            if want_amd_patch { AMD_PATCHES } else { empty },
        ];
        {
            for pt in lists.iter().flat_map(|l| l.iter()) {
                let dst = (pt.va - KBASE) as *mut u8;
                let mut same = true;
                for (i, b) in pt.expect.iter().enumerate() {
                    if *dst.add(i) != *b {
                        same = false;
                    }
                }
                if same {
                    for (i, b) in pt.replace.iter().enumerate() {
                        *dst.add(i) = *b;
                    }
                    com1("anathema-xboot: patched ");
                    com1(pt.why);
                    com1("\n");
                } else {
                    com1("anathema-xboot: patch skipped (bytes differ): ");
                    com1(pt.why);
                    com1("\n");
                }
            }
        }
        core::ptr::write_bytes(base as *mut u8, 0, (final_end - base) as usize);
        core::ptr::copy_nonoverlapping(dt.as_ptr(), dt_pa as *mut u8, dt.len());
        core::ptr::copy_nonoverlapping(kc.as_ptr(), kc_pa as *mut u8, kc.len());
        core::ptr::copy_nonoverlapping(est.as_ptr(), est_pa as *mut u8, est.len());
        core::ptr::copy_nonoverlapping(raw.as_ptr(), mmap_pa as *mut u8, copy_len);
        {
            let ds = meta.desc_size;
            let n = copy_len / ds;
            let bp = mmap_pa as *mut u8;
            let mut tmp = [0u8; 128];
            if ds <= tmp.len() {
                for i in 1..n {
                    let mut j = i;
                    while j > 0 {
                        let a = bp.add((j - 1) * ds);
                        let b = bp.add(j * ds);
                        let ka = core::ptr::read_unaligned(a.add(8) as *const u64);
                        let kb = core::ptr::read_unaligned(b.add(8) as *const u64);
                        if ka > kb {
                            core::ptr::copy_nonoverlapping(a, tmp.as_mut_ptr(), ds);
                            core::ptr::copy_nonoverlapping(b, a, ds);
                            core::ptr::copy_nonoverlapping(tmp.as_ptr(), b, ds);
                            j -= 1;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        let mut o = 0usize;
        while o + 32 <= copy_len {
            let ap = (mmap_pa as usize + o + 32) as *mut u64;
            *ap &= !(1u64 << 63);
            o += meta.desc_size;
        }
        ba[1032..1036].copy_from_slice(&(mmap_pa as u32).to_le_bytes());
        ba[1036..1040].copy_from_slice(&(copy_len as u32).to_le_bytes());
        ba[1040..1044].copy_from_slice(&(meta.desc_size as u32).to_le_bytes());
        ba[1044..1048].copy_from_slice(&meta.desc_version.to_le_bytes());
        ba[1144..1152].copy_from_slice(&phys.to_le_bytes());
        core::ptr::copy_nonoverlapping(ba.as_ptr(), boot_args_pa as *mut u8, 4096);
    }
    com1("anathema-xboot: kernel copied, jumping\n");
    unsafe { paint_band(4, 0x00ff_00ff); }
    unsafe { handoff(pml4_pa, gdtr_pa, kern.entry_pa as u32, boot_args_pa as u32) }
}
