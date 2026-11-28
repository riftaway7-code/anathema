# Anathema

Boot **macOS 27** on a regular PC with **our own boot loader**. No OpenCore. Anathema starts Apple's real Intel macOS 27 kernel from a UEFI loader, loads the kexts it needs, and brings up Apple's real userspace on top.

**Join us:** https://discord.gg/ZSQxYC8RT7 · [FAQ](FAQ.md) · [Contributing](CONTRIBUTING.md) · [TODO](TODO.md) · [ideas](https://github.com/riftaway7-code/anathema-ideas)

**This is fully vibe coded.** Every line was written by an AI (Claude Code) under human direction. It has bugs, it has not been audited, and a lot of it was found by trial and error against a real macOS kernel. Read it as a research log that happens to work.

## Where it stands (updated 2026-11-24)

Everything below was tested in QEMU (UEFI, USB stick, SATA and NVMe disks). **Nothing has been tried on a real PC yet except the early loader bring-up.**

Working:

* **Own UEFI loader** (`crates/anathema-xboot`) starts the macOS 27.2 Intel kernel (`xnu-13432.41.4`, build 26B131) from a USB stick: memory map, device tree, boot args, patches, and the UEFI framebuffer for on-screen boot text.
* **Kext collection built by us** (`tools/kcbuild.py` and `crates/anathema-kc`): links Apple's Intel kexts from the Tahoe and macOS 27 KDKs into one block the kernel loads. Storage (AHCI, NVMe, USB mass storage), USB, ACPI, APFS/HFS+, tmpfs and filesystem compression.
* **Real userspace**: Apple's launchd, syslogd, bash, dyld and the real dyld shared cache run on the kernel. The commpage is mapped by a loader hook, and a stand-in Sandbox policy keeps launchd's service lookups working.
* **Apple's recovery environment** (the x86_64 BaseSystem, APFS) boots and stays up.
* **CPUs**: Intel (Haswell to Cascade Lake in QEMU) and AMD (the loader detects it and patches CPU identification and timing; one core in QEMU).
* A `tools/` kit to build a GPT USB image: ESP with the loader, kernel and kexts, plus a root partition.

In progress:

* **Graphics and the installer window.** The NVIDIA GPU driver by NullMoth (RTX 20/30/40/50, separate noncommercial license) is being wired in; WindowServer and the installer need it.
* **USB keyboard and mouse** in the recovery environment.
* **Real hardware tests** (`crates/anathema-setup` builds the `anathema-xboot` USB stick ESP on Windows).

Not working yet: Wi-Fi, audio, Bluetooth, sleep, AMD multi-core in the emulator, and anything on real hardware beyond the loader bring-up.

## How it works

```
UEFI firmware -> anathema-xboot (loader) -> macOS 27 Intel kernel
                     |                          |
                     |  patches + stand-ins     |  our kext collection
                     v                          v
              commpage, sandbox,          storage, USB, APFS, ...
              amfi, runtime services            |
                                                v
                                   Apple's launchd, dyld, recovery
```

The loader gives the kernel what a Mac's firmware normally would (a fake EFI environment and kernel-collection header), applies a short list of byte patches for things the Intel kernel expects from Apple hardware, and adds small stand-ins in code space for pieces we can't run (trust cache, AMFI, sandbox). `TODO.md` lists what each stand-in should become.

## Try it / help

* Test on real hardware and report in the Discord `hardware-support` channel.
* Pick a task from [TODO.md](TODO.md) and say so in `dev-talk`.
* The repo contains no Apple software: the tools download what they need.

---

## Research track: arm64 translation (the original approach)

Anathema started as an attempt to boot the *arm64* macOS 27 VM kernel on an x86-64 PC by translating arm64 to x86-64. That work (the crates below) is kept as a research track; the native Intel path above is what's moving fast.

### What it is

Anathema targets the **VirtualMac2,1 (`vma2macosap`)** build of macOS 27.2. That is the kernel Apple ships for macOS virtual machines. It has no SPTM and no TXM, runs on a fairly plain virtual platform (GICv3, PL011 UART, PL031 RTC, virtio PCIe devices) and is the easiest arm64 macOS to bring up without Apple silicon.

The pieces:

| crate | what it does |
| --- | --- |
| `anathema-xlat` | AArch64 to x86-64 translator. Decoder, x86 emitter, block translator and runner. `no_std`. Fuzzed instruction by instruction against `qemu-user` as an oracle. |
| `anathema-rt` | The runtime: a virtual arm64 machine built around the translator, plus the `rt-boot` test harness and a `no_std` `Vm` library. |
| `anathema-boot` | A UEFI application: boot menu, hardware report, readiness check, bring-up test, and a "Boot macOS" entry that runs the runtime inside the loader. |
| `anathema-media` | IPSW tooling: update feed and ranged download, zip, plist, IM4P, AEA decryption, APFS reader, dyld shared cache reader. |
| `anathema-setup` | Windows installer that prepares the USB stick (targets `anathema-xboot` ESP). |

### The translator (`anathema-xlat`)

* Decodes arm64 (integer, FP, a large part of NEON, atomics, exclusives, crypto) and emits x86-64.
* On the macOS kernel's `__TEXT_EXEC` the output measured about 9.7 bytes of x86 per arm instruction, and translation ran at roughly 106 to 128 ns per instruction in our benchmark. These are our numbers on our machine, nothing more.
* Every translated instruction is checked against `qemu-user` with random state (`rt-boot fbcheck`). Several real bugs were found this way (SHA-256/512 schedule mistakes, flag handling around indirect branches, and others).
* The runtime uses the "exit" memory model: every load and store leaves translated code and goes through a software MMU. That is simple and correct and also slow. Faster memory models are future work.

### The runtime (`anathema-rt`)

A virtual machine around the translator:

* software MMU with a 16K granule walker, TLB, permission and access-flag faults, AT/PAR_EL1, DC ZVA
* EL0 and EL1, exception entry and `eret`, `svc`, SP_EL0/SP_EL1 banking, PAN, exclusive monitor
* GICv3 (distributor, redistributor, ICC system registers, MSI frame) and the ARM generic timer, with Apple style FIQ timer delivery
* PL011 UART (the kernel's verbose boot log comes out here), framebuffer
* PCIe host (ECAM) with a virtio-blk device, MSI-X into the GIC
* the Apple AES engine (key, IV and data commands over the FIFO, real AES with DMA)
* NVRAM image, boot_args and device tree builder, Mach-O loader with chained fixups, RAM disk hand-off
* hand-written NEON/crypto fallbacks for instructions the translator does not cover yet, all verified against qemu

`rt-boot` is the Linux harness. It has a lot of env-var debug knobs (`RT_HIST`, `RT_EXC`, `RT_RDWATCH`, `RT_WSEARCH`, ...). They are documented only by the code.

### The loader (`anathema-boot`)

* UEFI application for a USB stick.
* Hardware report and readiness check (ACPI, PCI, CPU features, display).
* Bring-up test: own GDT, IDT, paging, APIC, SMP (all cores), AMD-Vi takeover, read-only NVMe and GPT reader. This ran on a real Ryzen 5 5500.
* **Boot macOS** entry: loads the kernel and device tree from the ESP, reads the RAM disk from an exFAT data partition with a built-in read-only exFAT reader, runs the runtime and blits the guest framebuffer to the screen with the kernel log underneath.

### Status of the arm64 research track

What works, measured:

* The vma2 kernel (`xnu-13432.41.4`, `RELEASE_ARM64_VMAPPLE`) boots under the translator in the Linux harness, through early boot, the FIPS self test, NVRAM, IOKit startup, PCI enumeration, the AES accelerator, AMFI and the sandbox policies.
* The kernel finds our virtio disk, and the restore RAM disk (`141-12288-175.dmg` from the IPSW) is recognised as the boot device. APFS mounts it as the root filesystem.
* The kernel then tries to run `/sbin/launchd` as PID 1.

Where it stops:

* `exec` of `/sbin/launchd` fails with `errno 8` (exec format error) and the kernel panics. The binary is present and valid in the image, so the suspect is how the kernel reads it back, not the file. This is the current work item.
* So no user-mode code has run yet. All the arm64 that has run so far is kernel code.

What does not work or does not exist yet:

* **Graphics.** macOS VMs use Apple's paravirtualized GPU. Its guest protocol is private to Apple's host framework. The kernel talks to it through a command ring and waits for answers we cannot give. For now the device is removed from the device tree and the kernel runs headless. There is no desktop and no installer UI, and getting one would need an equivalent of Apple's host-side renderer, which is a very large project.
* **Speed.** All memory access is interpreted. The harness takes minutes to reach the point above. Real performance work is still ahead.
* One CPU only. No networking, USB, audio, or sound.
* The "Boot macOS" loader path has only been exercised under QEMU/OVMF, not on a real PC yet.

## Building

```
# harness (Linux)
cargo build --release -p anathema-rt
# fuzz the arm64 emulation against qemu-user (needs qemu-aarch64 and cross libs)
./target/release/rt-boot fbcheck 2000

# UEFI loader
rustup target add x86_64-unknown-uefi
cargo build --release -p anathema-boot --target x86_64-unknown-uefi
# result: target/x86_64-unknown-uefi/release/anathema-boot.efi
```

The toolchain is pinned in `rust-toolchain.toml`.

## Testing

Unit tests run on any host, including Apple silicon Macs. They need no Apple software, no QEMU and no network:

```
cargo test -p anathema-media                   # IPSW parsers: plist, zip, IM4P/DER, LZSS, device tree
cargo test -p anathema-xlat                    # arm64 decoder and x86-64 emitter (the QEMU diff test is skipped off x86-64 Linux)
cargo test -p anathema-rt --test mmu_and_memory   # software MMU, guest loads/stores, device tree builder
```

The parser tests feed each parser corrupt input as well as good input. A bad file must give an error, never a panic.

Tests marked `#[ignore = "known bug: ..."]` describe a bug that is known and not fixed yet. `cargo test -- --ignored` runs them, and they should fail until the bug is fixed. Remove the `ignore` in the same change as the fix.

Two kinds of check need more:

* `cargo test -p anathema-xlat --test diff` (built only on x86-64 Linux) and `rt-boot fbcheck` run translated code and compare it with `qemu-aarch64`. They need an x86-64 Linux host with `qemu-aarch64` and `aarch64-linux-gnu-objdump` installed.
* The boot path itself (the harness reaching `launchd`) needs your own IPSW, see below.

### Checking the translator against real arm64 hardware

QEMU is a good oracle, but it is still an emulator. `anathema-hwcheck` uses a real Apple silicon CPU instead. It generates random programs with the same generator as the QEMU diff tests, runs each one directly on the Mac's CPU, and saves inputs and outputs to a corpus file. The same programs are then replayed through the translator on x86-64 Linux, and every difference is printed.

On an Apple silicon Mac:

```
cargo run --release -p anathema-hwcheck -- selftest                       # checks the recording harness itself
cargo run --release -p anathema-hwcheck -- record --cases 5000 --out hw-corpus.bin
cargo run --release -p anathema-hwcheck -- info hw-corpus.bin             # counts, and instructions the CPU refused
```

Copy `hw-corpus.bin` to an x86-64 Linux machine, then:

```
cargo run --release -p anathema-hwcheck -- replay hw-corpus.bin --model identity
```

`record --fp-specials` seeds the vector registers with ±0, ±infinity and quiet and signalling NaNs, which the stock generator never produces. `--model` can be `identity`, `kernel` (the runtime's register pinning), `biased` or `checked`. `--strict` also counts differences that are only in NaN payloads or signs, which the default comparison hides.

A case the CPU rejected with SIGILL but the translator ran anyway means the decoder accepts an encoding that real hardware treats as undefined. Those are reported too. Corpora aren't committed (`hw-corpus*.bin` is gitignored). Recording 5000 cases takes well under a second.

## Contributing

Issues and patches are welcome, and outside review especially. If you find a bug, an issue with the file and line and a short explanation is already a big help. For anything larger, open an issue first so work doesn't overlap. New tests are always welcome, especially for parsers and the instruction decoder.

## Running the harness

The repo contains no Apple software. You need a macOS 27 IPSW that you obtained yourself and you need to extract from it:

* `kernelcache.release.vma2` (an IM4P holding a Mach-O)
* `DeviceTree.vma2macosap` (an IM4P)
* the restore RAM disk (`141-12288-175.dmg`, an IM4P holding a raw APFS container)

`anathema-media` can read the IPSW and unwrap IM4P files. Then:

```
RT_VIDEO=1 RT_RAMDISK=ramdisk.bin RT_BOOTARGS="-v serial=3 debug=0x14e rd=md0" \
  rt-boot kc-vm.macho dt-vm.bin
```

The kernel log appears on stdout. The preparation steps are rougher than they should be and the docs are thin. Issues and patches welcome.

## Legal

Anathema contains no Apple code, firmware or images. macOS is licensed by Apple for use on Apple branded hardware. This repository is research into how the platform works. What you do with it is your responsibility.

## Credits and license

MIT, see `LICENSE`, with one exception: `crates/anathema-rt/src/aesdev.rs` follows the command and register behaviour of QEMU's `hw/vmapple/aes.c` (GPL-2.0-or-later) and should be treated as GPL-2.0-or-later. QEMU's vmapple machine was also the reference for what the platform looks like. Stela is a separate project and was looked at only for ideas. Anathema is not a fork of it.
