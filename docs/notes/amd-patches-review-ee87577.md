# Review: AMD patches in ee87577

Commit: `ee87577 xboot: boot on real ryzen b550: amd patches, cpu acpi table, pci 10-bit tag fix`
File: `crates/anathema-xboot/src/main.rs` (+518 / -40). Reviewed by reading only; nothing was run on
hardware or in QEMU, and the kernel was not disassembled. Statements about what the kernel code does
come from the `why` strings, the byte patterns, and public XNU sources (`osfmk/i386/cpuid.c`,
`cpu_threads.c`, `cpu_topology.c`), which may differ from xnu-13432 / 26B131. Those are marked
"(verify)".

## Summary

The B550 boot works by making the kernel see a **single-core, single-thread package**: the
`cpus=1` default boot-arg, MSR 0x35 (core/thread count) forced to 0, and the two cache-sharing loads in
the topology code forced to 1. Those last two patches are a workaround for the same divide-by-zero
listed in TODO.md under "AMD multi-core". So the multi-core issue is not independent of this commit:
the commit papers over it in a way that only holds while there is one CPU.

Highest-risk findings, in order:

1. The new hardware dump reads AMD-only MSRs unconditionally, so it will likely #GP on Intel CPUs.
2. The new `cpuid_set_cpufamily: force PENRYN` patch is dead code: it targets the same site that
   `install_amd_cpuid_spoof` rewrites first, so it is always skipped.
3. Several patches have 1-2 byte `expect` guards and there is no check that the kernel is the pinned
   build, so a different kernel could get patched in wrong places silently.
4. Patch results only go to COM1. A B550 board without a serial header cannot see which patches
   were skipped.

## Patches that overlap with AMD multi-core

These are the pieces a multi-core fix will have to touch or replace. Today they are all hard-wired to
"1 core, 1 thread".

| va | patch | multi-core relevance |
| --- | --- | --- |
| `0x3e80ee`, `0x3e8102` | `mov ecx,[rax+0xdc]` / `[rax+0xe0]` -> `mov ecx,1` ("shared cache must say 1 or it divides by zero") | **This is the divide-by-zero from TODO.md.** It pins two per-package/cache-sharing fields to 1 so the topology division can't hit 0. For multi-core they must carry the real values instead, consistent with the counts below. Identify which `i386_cpu_info` fields `0xdc`/`0xe0` are (verify; likely `cpuid_cores_per_package` / `cpuid_logical_per_package` or `core_count` / `thread_count`). |
| `0x3e5a89`, `0x3e5aa5` | `rdmsr; or edx,eax` (MSR 0x35) -> `xor eax,eax; xor edx,edx` | The kernel's core/thread count input. Note the `why` says "report 1 core 1 thread" but the code reports **0 and 0**. In public XNU a zero `core_count` falls back to `cpuid_cores_per_package` / `cpuid_logical_per_package` (verify). For multi-core this needs `(cores << 16) \| threads` from CPUID 0x80000008 / 0x8000001E, which needs 5+ bytes (`mov eax, imm32`), so it won't fit the 4-byte site; use a cave the way `install_amd_cpuid_spoof` does. |
| `0x3e65e3` | cache leaf 4 -> 0x8000001D | Sets the cache sharing counts. On AMD, leaf 0x8000001D EAX[31:26] is reserved (0), so the kernel's "cores per package" from this leaf comes out as 1. That is why OpenCore's amd_vanilla also has separate "force cores per package" patches with a per-CPU core count byte. Those are **missing here** and would be needed for multi-core. |
| `DEFAULT_ARGS` `cpus=1` | boot-arg | Hides AP start-up entirely. Any multi-core test must drop it (or set `xargs.txt`). |
| `add_amd_cpu_ssdt` | SSDT with `Processor` objects for up to 32 threads | Already multi-core ready: it is the ACPI side macOS needs to enumerate the extra CPUs. Risks are listed below (ProcID mapping, thread count). |
| `install_amd_cpuid_spoof` (pre-existing) | Haswell family 6 / model 0x3C / stepping 3 | Haswell's topology path is the one that will run, so the multi-core fix should be checked against what the kernel does for `CPUFAMILY_INTEL_HASWELL`. |

Not related to topology: the TSC MSR reads (0x198/0x194/0xCE), SYSENTER wrmsrs, XCPM returns, the
mitigation return, MSR 0x8B / 0x17, leaf 7, the version check, PAT, the i386_init / i386_vm_init rdmsrs,
and the PCI 10-bit tag change. These should carry over unchanged to a multi-core boot, but each one
runs on the boot CPU only today. Any that patch per-CPU init paths, such as `cpu_syscall_init` and
`mtrr_update_action`, will also run on every AP once multi-core works. They are idempotent text patches,
so that should be fine.

Suggested shape for the multi-core fix: compute `cores` and `threads` at load time from CPUID
0x80000008 ECX[7:0]+1 and 0x8000001E EBX[15:8]+1 (both already dumped to `boot.log`). Then feed them
consistently to the MSR 0x35 site, the two `0x3e80ee` / `0x3e8102` sites, and a new cores-per-package
site, and test in QEMU `-cpu EPYC -smp 2` first. `Patch` is `&'static [u8]` today, so it needs a
variant whose bytes are built at runtime.

## Risks and bugs

### 1. AMD-only MSR reads run on every CPU (high)

The `boot.log` hardware dump (before `exit_boot_services`) is not gated by `is_amd`. It reads
`SYSCFG` (0xC0010010) and `HWCR` (0xC0010015), which are AMD MSRs, and `TSC_AUX` (0xC0000103), which
needs RDTSCP. On an Intel CPU `rdmsr` of an unimplemented MSR raises #GP. The firmware's exception
handler then hangs or resets, before the loader reaches the kernel. Under KVM the same happens unless
`ignore_msrs` is set. This could break the Intel / QEMU-Intel path that the README says works.
Suggested fix: read those three only when `is_amd`, and guard `TSC_AUX` on CPUID 0x80000001 EDX[27].

### 2. The PENRYN patch is dead and conflicts with the spoof (medium)

`AMD_PATCHES` entry `0x3e58a5` (`cmp byte [cpuid_family],6; jne` -> force `CPUFAMILY_INTEL_PENRYN`)
targets the same address as `AMD_SITE_PA` in `install_amd_cpuid_spoof`. The spoof runs first and
writes `e9 ...`, so the PENRYN patch's `expect` never matches and it is always logged as "patch
skipped". The kernel ends up Haswell, not Penryn. Pick one and delete the other. If both are ever
reordered, the PENRYN bytes would overwrite the cave jump and the stepping/model spoof would be lost.

### 3. Weak guards and no kernel identity check (medium)

All patches use fixed VAs for 26B131. The guards for `0x3e5e00`, `0x41bc20` and `0x41c700` are a
single `0x55` (`push rbp`), which almost every function starts with. The guards for the three
SYSENTER sites are just `0f 30`. With a different kernel these could turn some other function into
`ret`. Suggest checking the kernel's `LC_UUID` (or a hash of `__TEXT`) once and skipping the whole AMD
list if it doesn't match, instead of relying on per-patch bytes.

### 4. Skipped patches are invisible on real hardware (medium)

Patch results go only to `com1()`. `boot.log` is written before `exit_boot_services`, so it can't contain
them. On a board without a serial header a skipped patch only shows up as a later hang. Suggest
working out patch applicability before `exit_boot_services` (the kernel image is already in memory
then, at a known offset) and writing it to `boot.log`. Or paint one marker band if any AMD patch was
skipped.

### 5. PAT patch drops write-combining (low now, matters for graphics)

`mtrr_update_action` now writes PAT = `0x0007010600070106` (the power-on default: WB, WT, UC-, UC,
twice). The kernel wanted PA6 = WC. So every mapping the kernel asks to be write-combining, such as the
framebuffer and later GPU BARs, becomes UC-. It's correct and safe, but the framebuffer console and any
future WindowServer / NVIDIA work will be noticeably slower. That is what amd_vanilla does too.
Revisit once graphics is up: only the value differs, so `0x0007010600070106` could likely become
`0x0001010600070106`-style with PA6 = WC (verify the bit layout).

### 6. i386_init / i386_vm_init: argument setup removed too (low, verify)

The three 42-byte replacements NOP out the two `rdmsr` (0x199, 0x198) and also `mov edi,0x05310258;
xor ecx,ecx; xor r8d,r8d`. That looks like argument setup for a following call, probably a kdebug
trace with code 0x05310258. If that call is still there, it runs with garbage args. That is harmless
if kdebug is off at that point. Check the bytes after +42 against amd_vanilla's replacement.

### 7. XCPM / mitigation `ret` patches (low, verify)

`push rbp` -> `ret` leaves `rax` at whatever the caller had. That's fine if `xcpm_bootstrap`,
`xcpm_init` and the mitigation setup return `void`. Check the callers don't test the result.

### 8. ACPI CPU table (`add_amd_cpu_ssdt`)

The AML matches dortania's SSDT-CPUR. I checked it by hand: Processor PkgLength 27, Scope 2-byte
PkgLength with a 4095 limit (32 threads gives 935), header fields and checksum, and the XSDT checksum.
Risks:

- **ProcID = 0..n-1.** macOS ties `Processor` objects to MADT Local APIC entries by ACPI processor
  UID. If the B550 MADT UIDs are not 0..n-1, the extra CPUs won't match. This doesn't matter with
  `cpus=1`, but it will for multi-core.
- **Thread count = number of `ACPI0007` strings**, capped at 32, last matching table wins. AGESA often
  declares the socket maximum (some with `_STA` = 0), so a 6-core 5500 may get 16-32 Processor objects.
  That is probably harmless, but it's another multi-core variable. The MADT count of enabled LAPICs
  would be more exact.
- **`has_processor` is a raw search for bytes `5b 83`** anywhere in DSDT/SSDT bodies, so a false hit
  skips the table (only `xacpiforce` overrides it).
- `&t[..4]` panics (hang) if an XSDT entry points at a table whose length field is < 4.
- The RSDP is rewritten in place. That's fine if it's in RAM. If firmware maps it read-only (memory
  attributes table), the write faults. A safer variant is a new RSDP in ACPI_RECLAIM plus updating the
  config table entry that `fake_system_table` copies.

### 9. Early markers / boot trail (low)

- `LOG_PA = 0x4000000` (64 MB): the marker hooks write to `KBASE + LOG_PA`. Nothing reserves that
  range or checks it against `[base, final_end)`. If the kernel + KC + other blobs ever grow past
  64 MB, or the kernel allocates there early, the markers corrupt memory. Check it, or allocate the
  page from the firmware.
- `tools/mkmark.py`, which `install_markers` cites as the blob format, is **not in the repo**, so
  `xmark.bin` can't be rebuilt or reviewed.
- The boot trail uses CMOS register 0x01 (RTC seconds-alarm). It's mostly harmless, but it is a real
  RTC register; an unused CMOS byte would be cleaner.
- `install_markers` reads `blob[o + ...]` without bounds checks. A truncated `xmark.bin` panics after
  `exit_boot_services` (silent hang). `clen > 160` returns early and skips the remaining markers
  silently.

### 10. Non-AMD changes in the same commit (look fine)

- **Handoff trampoline** copied to a page below 4 GB: needed when firmware loads the image above
  4 GB, because the 32-bit `retfq` target must be < 4 GB. The page is covered by the 4 GB identity map
  and the register mapping matches sysv64 (rdi, rsi, rdx, rcx). Fine.
- **Memory map insertion sort** by `phys_start` before hand-off: correct, O(n^2) on a few hundred
  entries, `desc_size <= 128` guarded.
- **GOP `GetProtocol`** instead of `open_protocol_exclusive`: avoids disconnecting the firmware
  console on real boards. Fine.
- **PCI 10-bit tags**: signature scan over the KC with a "exactly one hit" guard, `jae` -> `jmp`. Good
  pattern. It's the model the fixed-VA patches should move towards.
- `boot.log` is written to the stick on every boot, plus a 4 s stall. Expected, just noting the write.

## Suggested follow-ups (not done)

1. Gate the AMD MSR reads in the hardware dump on `is_amd` (risk 1). Small, and it unblocks Intel.
2. Delete the dead PENRYN patch or the spoof (risk 2).
3. Kernel UUID check before applying any fixed-VA list (risk 3).
4. Runtime-valued patches for cores and threads, then QEMU `-cpu EPYC -smp 2` without `cpus=1`
   (multi-core).
