# TODO

Open work, roughly in order. Pick one and say so in `dev-talk`.

## Getting to the installer
- [ ] **Sandbox policy.** Loading the Sandbox kext panics ("shenanigans", `evaluate.c`). It needs the platform sandbox profile. Without it launchd denies many service lookups.
- [ ] **Trusted services.** launchd rejects plists with `_Conclave` (for example WindowServer) because there is no trust cache. We strip the key for now; a proper trust-cache story is better.
- [ ] **Power management.** `pmset`/powerd have nothing to talk to, so the recovery system reboots. We stub `pmset` for now.
- [ ] **Graphics.** A framebuffer driver and then the NVIDIA driver (built on NullMoth's macOS 27 driver) so WindowServer can draw.
- [ ] **USB keyboard and mouse** (HID) in the recovery environment.
- [ ] **Installer app** launching on top of the above.

## Platform
- [ ] **AMD multi-core.** In QEMU the kernel divides by zero while sizing the CPU topology on the EPYC model; check real Ryzen.
- [ ] **Real hardware.** First boots on real boards, NVMe and USB, with notes in `hardware-support`.
- [ ] **AMFI.** The AMFI stand-in returns 0 everywhere; a more faithful stub or the real thing would help.
- [ ] **NVRAM and EFI runtime.** Proper runtime services instead of the stub table.
- [ ] **Wi-Fi, audio, Bluetooth.** Not started.

## Tooling
- [x] Rust port of `tools/kcbuild.py` (`crates/anathema-kc`).
- [x] An installer/setup tool (`crates/anathema-setup`) that downloads KDKs, links `xkc.bin`, and writes the `anathema-xboot` ESP on Windows (full root/payload image staging stays in `tools/`).
- [ ] Faster test loop: parallel emulator runs, disk snapshots.
