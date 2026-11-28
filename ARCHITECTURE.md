# Architecture

This document explains how the pieces of Anathema fit together, where to find them, and which parts are workarounds. It is meant for people who want to read or change the code.

It was written from the source at commit `ac6f452` (24 November 2026). Where this file and the README or FAQ disagree, trust the code, then this file. See [Docs that are behind](#docs-that-are-behind).

## Two boot designs in one repo

Anathema contains two different ways of getting macOS 27 to start on a PC. They share tooling but not a boot path.

| | Design A: translate the arm64 kernel | Design B: boot the x86-64 kernel |
| --- | --- | --- |
| Kernel | `VirtualMac2,1` (`vma2`) arm64 kernel | Apple's x86-64 macOS 27 kernel |
| How it runs | Translated to x86-64 at run time, inside a software virtual machine | Runs natively on the PC's CPU |
| Main crates | `anathema-xlat`, `anathema-rt`, `anathema-boot` | `anathema-xboot`, `anathema-kc`, `anathema-setup`, and `tools/` |
| Described in | README (research track), FAQ | README, this file |

Recent commits are almost all Design B (`xboot`, `anathema-kc`, `anathema-setup`, and `tools/`). Design A is kept in the tree as a research track and is still what [FAQ.md](FAQ.md) describes.

## Crates

| Crate | What it is |
| --- | --- |
| `anathema-xlat` | AArch64 to x86-64 translator. `decode.rs` decodes arm64, `x86.rs` emits x86-64, `trans.rs` (with `trans/vec.rs` and `trans/memops.rs`) translates blocks. `no_std`. Checked instruction by instruction against `qemu-user`. |
| `anathema-rt` | Design A runtime: a virtual arm64 machine built around the translator (software MMU, GICv3, timer, PL011 UART, PCIe with virtio-blk, Apple AES engine, device tree builder, Mach-O loader), plus the `rt-boot` Linux harness. |
| `anathema-boot` | Design A UEFI application with a boot menu, hardware report, readiness check (`readiness.rs`), and a bring-up test (`bringup/`: GDT, IDT, paging, APIC, SMP, AMD-Vi in `iommu.rs`, read-only NVMe in `nvme.rs`, GPT in `gpt.rs`). Its "Boot macOS" entry runs the Design A runtime. |
| `anathema-xboot` | Design B loader. A single-file `no_std` UEFI application (`src/main.rs`) that loads the x86-64 kernel and starts it. Details below. |
| `anathema-kc` | Rust port of `tools/kcbuild.py`. Links Apple Intel kexts from KDK `Extensions` directories against the kernel into `xkc.bin` (plus `.map` and link report). Tested for parity via `tools/kc_parity.sh`. |
| `anathema-hwcheck` | Checks the translator against real arm64 hardware: record on an Apple silicon Mac, replay on x86-64 Linux. |
| `anathema-media` | IPSW and KDK tooling: update feed, ranged download, zip, plist, IM4P, AEA decryption, APFS reader, dyld shared cache reader, and KDK unpacking (`kdk/`: UDIF, HFS+, xar, pbzx, cpio). |
| `anathema-setup` | Windows installer GUI (`src/app.rs`) and `anathema-esp` CLI (`src/bin/esp.rs`) targeting the Design B ESP layout. Embeds `anathema-xboot` (`EFI/BOOT/BOOTX64.EFI`) and `assets/xcp.bin`, downloads and unpacks the pinned Tahoe and macOS 27 KDKs, links `xkc.bin` with `anathema-kc`, writes `anathema/{xkernel,xkc.bin,xcp.bin}` to a 200 MiB FAT32 ESP, and formats the rest of the stick as exFAT. |

## Design B: how `anathema-xboot` starts the kernel

`xboot` is a UEFI application. It does the job a bootloader does on a real Mac or Hackintosh, with its own patches in place of firmware support the kernel expects.

### Files it reads

All files come from `\anathema\` on the EFI system partition.

| File | Purpose |
| --- | --- |
| `xkernel` | The macOS 27 x86-64 kernel (Mach-O). Required. |
| `xargs.txt` | Boot arguments. If missing, the default is `-v serial=3 debug=0x14e cpus=1 kt_fixed_zones=200 kt_var_heaps=6`. |
| `xkc.bin` | A block of kexts linked offline by `tools/kcbuild.py` or `anathema-kc`. Optional; its header and layout are described in a comment in `crates/anathema-xboot/src/main.rs`. |
| `xcp.bin`, `xcp2.bin` | Small machine-code blobs for the commpage work (see below), built from `tools/commpage_stub.c` (via `tools/mkxcp.sh`) and `tools/commpage_enter.c`. |
| `xcp3.bin` | Small machine-code blob built from `tools/loadkexts.c` that registers the sandbox stand-in policy and loads personality-less kexts right before `launchd` starts. |
| `xpatch-amd`, `xpatch-spoof` | Flag files. Their contents are ignored; their presence forces the AMD CPU patches on non-AMD CPUs (`xpatch-tsc` is also checked, though TSC measurement runs unconditionally). |

### What happens, in order

1. Prints to the serial port (COM1) with the prefix `anathema-xboot:`. Every later step logs there too.
2. Reads the files above and parses the kernel's Mach-O headers.
3. Detects the CPU. If the vendor string is `AuthenticAMD`, the AMD patches are applied automatically; `xpatch-amd` or `xpatch-spoof` force them on other CPUs.
4. Measures the TSC against the firmware timer and works out the ratio in units of the 100 MHz bus. The kernel normally reads this from `MSR_PLATFORM_INFO`, which QEMU leaves at zero and AMD CPUs do not have. This is always done.
5. Builds a minimal device tree: `/chosen` (`memory-map`, `random-seed`), `/efi/platform` (`FSBFrequency`, `system-id`), `/options` (`platform-uuid`), and `/efi/configuration-table/<GUID>` (one node per firmware configuration table so the kernel's ACPI driver can find the ACPI and SMBIOS tables).
6. Prepares an identity map of the first 4 GB (2 MB pages) and a small GDT, both below 4 GB. Stages `boot_args` and passes the UEFI framebuffer to the kernel console.
7. Exits UEFI boot services, and the memory map is patched into `boot_args`.
8. Applies the in-memory patches and stand-ins (next section), copies the kernel into place and jumps to its entry point.

### Patches and stand-ins

The loader changes the kernel in memory instead of emulating the missing pieces. Each one exists because the kernel was built for real Apple hardware and expects something this PC does not provide.

| What | Why it exists |
| --- | --- |
| Kernel collection header | A minimal `MH_FILESET` header (one segment, one `com.apple.kernel` entry) so the kernel takes its boot-kernel-collection code paths. It needs an empty `__PRELINK_INFO` (populated when `xkc.bin` provides plist data) and `__LINKEDIT` or it crashes. The kext block from `xkc.bin` is declared through its own `__PRELINK_TEXT` segment. |
| Stub EFI runtime services table | Valid signature and checksum, every service returns `EFI_NOT_FOUND`. Lets the NVRAM driver start without real runtime services, which cannot be mapped yet. |
| AMFI stand-in | The kernel calls through a struct of AMFI function pointers (`AMFI_GLOBAL_PA`). Every entry returns 0 (no entitlement present, nothing denied). |
| Platform UUID | The kernel waits forever for the platform UUID, which on x86 is published only once the NVRAM driver registers. Until NVRAM has real runtime services, the loader marks that step done. |
| Early VM stub & PV repair | The stock kernel runs its early memory reservations before `virtual_space_start/end` are set and hits PV/bounds issues on early maps. The stub asks `pmap_virtual_space()` for the real range first and repairs early PV tracking. |
| Commpage (`xcp.bin`, `xcp2.bin`) | The x86-64 macOS 27 kernel has no code that creates the commpage maps or maps the commpage into exec'd user tasks. `xcp.bin` creates the maps and `xcp2.bin` hooks `vm_map_exec` and the fixed locator to map them. |
| Kext starter & Sandbox stand-in (`xcp3.bin`) | Hooked at `load_init_program` right before `launchd` starts (`tools/loadkexts.c`). Calls `mac_policy_register` to register a `"Sandbox"` stand-in MAC policy whose syscall hook returns 0 (tables placed in the last 8 KB of the plist reserve at `SBX_VA`), and calls `OSKext::loadKextWithIdentifier` for kexts without matching personalities (`tmpfs` and the filesystem compression types). |
| Runtime skips & collection flags | Skips `commpage_update_mach_approximate_time`, `commpage_update_mach_continuous_time`, `arcade_ast`, `x86_64_protect_data_const`, `trust_cache_runtime_init`, and (when no `xkc.bin` is present) the boothash salt check; marks the standalone kernel header as part of a static collection. |
| AMD CPU identification | The kernel refuses non-Intel CPUs and wants an Intel family and model. The loader skips the vendor check, adds a cache-line-size fallback (Intel's CPUID leaf 4 does not exist on AMD) and overwrites the decoded family, model, and stepping with Haswell values (family 6, model `0x3c`, stepping 3). |

### How patching is made safe

Every patch is a `Patch { va, expect, replace, why }`. The loader writes `replace` only if the bytes at `va` match `expect`, and logs `patch skipped (bytes differ)` otherwise. Many addresses are fixed constants (`AMFI_GLOBAL_PA`, `CP_PA`, `CPUID_FAMILY_PA` and others), so they belong to one specific kernel build. A different kernel build makes patches skip (and the log says so), and the stand-ins written at fixed addresses are only valid for the build they were derived from. When a new kernel misbehaves, check the serial log first and expect to re-derive the addresses.

## Building and running a Design B test disk

The scripts in `tools/` (along with `anathema-esp` in `crates/anathema-setup`) build bootable disk/ESP images and run them in QEMU. A boot takes several minutes in the emulator.

| Script / tool | Role |
| --- | --- |
| `xrun.sh`, `xrun_usb.sh` | Build the loader, make a test disk, boot the Intel kernel in QEMU, then read the kernel's panic text. |
| `xrun_esp.sh` | Boot a standalone FAT32 ESP image (such as one built by `anathema-esp`) as a USB stick in QEMU and print the debug buffer and serial log. |
| `mkkc.sh`, `kcbuild.py`, `kc_parity.sh` | Link Intel kexts into one prelinked block (`xkc.bin`) without `kmutil`; `kc_parity.sh` verifies byte-for-byte parity between `kcbuild.py` and `anathema-kc`. |
| `mkxcp.sh` | Compiles `tools/commpage_stub.c` into `crates/anathema-setup/assets/xcp.bin`. |
| `mkdisk.sh`, `mkdisk3.sh`, `mkdisk4.sh` | Build a GPT disk image. `mkdisk3.sh` makes an ESP, a helper HFS+ root and an APFS container; `mkdisk4.sh` adds an `anathema-payload` HFS+ partition for GPU driver files (with the helper partition optional). |
| `mkroot.sh` | Fills an HFS+ image from a tar of a root tree through a helper Linux VM. |
| `patchroot.sh`, `patchroot2.sh` | Run as launchd daemons in the helper HFS+ root. `patchroot.sh` mounts and patches the APFS recovery volume; `patchroot2.sh` wires in the `pmset` stub, strips `_Conclave` keys, and installs `mountpayload.sh`/`payload.plist` and symlinks for the GPU payload. |
| `finalize.sh`, `startws.sh` | `finalize.sh` packages the patched APFS volume and payload partition via `mkdisk4.sh`, test-boots it once, and copies the image; `startws.sh` kickstarts `WindowServer`. |
| `rundisk.sh`, `rundisk2.sh` | Boot a full disk image from an emulated USB stick. `rundisk2.sh` is parallel-safe: `SNAP=1` discards disk writes so several runs can share one image. Environment variables: `QCPU`, `QSMP`, `WAIT`, `IMG`, and `TAG` for `rundisk2.sh`. |
| `mkfill.sh` | Installs a given file as `/sbin/launchd` in an image. |
| `mkinit.py`, `mkinit_console.py`, `mkinit_commpage.py` | Build tiny static x86-64 Mach-O programs that print a line from user mode, usable as a stand-in `launchd` to check that user code runs at all. |
| `udif.py`, `yaaextract.py`, `pbzxcat.py`, `ridiff_probe.py`, `kdk_*.py` | Helpers that unpack Apple's formats (UDIF disk images, YAA archives, pbzx streams, RIDIFF patches, KDK packages). |

Do not commit Apple binaries or keys. The build tools fetch what they need.

## Known gaps

The open work is tracked in [TODO.md](TODO.md). The ones that shape the design:

- **Security is stubbed.** AMFI returns 0 everywhere, the trust cache is missing (launchd rejects `_Conclave` plists, so the key is stripped), and the NVRAM and EFI runtime services are stubs.
- **Power management and real sandbox policy are missing.** `pmset` and powerd have nothing to talk to (stubbed), and the real Sandbox kext still panics without the platform sandbox profile (a minimal stand-in policy registers via `xcp3.bin` so `launchd` service lookups work).
- **No graphics path past the UEFI framebuffer yet, and no USB keyboard or mouse input.** Wiring in NullMoth's NVIDIA driver and HID input for the recovery environment is in progress.
- **AMD multi-core.** In QEMU the kernel divides by zero while sizing the CPU topology on the EPYC model. The default `cpus=1` boot argument sidesteps it.
- **Real hardware.** Design B has not been booted on real boards yet beyond early loader bring-up. Reports from real boards are the most useful contribution, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Docs that are behind

- [FAQ.md](FAQ.md) was written for Design A: it says the kernel stops when `exec` of `/sbin/launchd` fails with `errno 8` and that Anathema is "not a Hackintosh".
- [TODO.md](TODO.md) still leaves the Rust `kcbuild.py` port and the USB setup tool unchecked under Tooling even though `anathema-kc` and `anathema-setup` now cover both.
- [README.md](README.md) was updated on 24 November 2026 to lead with Design B (with a couple of `"open PR"` notes for `anathema-kc` and `anathema-setup`, which have since landed) and moves Design A into its research-track section.

## About this document

Updated by reading the source at commit `ac6f452`, not by running it. The loader and Design B tooling descriptions come from `crates/anathema-xboot/src/main.rs`, `crates/anathema-kc/src/`, `crates/anathema-setup/src/`, `crates/anathema-setup/build.rs`, `crates/anathema-media/src/lib.rs`, `crates/anathema-media/src/kdk/mod.rs`, and the scripts in `tools/`. The `anathema-xlat`, `anathema-rt` and `anathema-boot` descriptions come from `README.md`, `lib.rs` / `main.rs` entry points, and the crate layout. If something here is wrong, the code is right.
