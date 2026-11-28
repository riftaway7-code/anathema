# TODO

Open work, roughly in order. Pick one and say so in `dev-talk`.

## Getting to the installer
- [ ] **WindowServer on real hardware.** The current blocker: launchd's `posix_spawn` of WindowServer fails with errno 2 (`ENOENT`). Suspected cause: the macOS 27 kernel running with a userspace from a different build. Not confirmed.
- [ ] **Real Sandbox policy.** A stand-in Sandbox policy in the loader now keeps launchd's service lookups working. Loading Apple's Sandbox kext still panics ("shenanigans", `evaluate.c`): it needs the platform sandbox profile.
- [ ] **Trusted services.** launchd rejects plists with `_Conclave` (for example WindowServer) because there is no trust cache. We strip the key for now; a proper trust-cache story is better.
- [ ] **Power management.** `pmset`/powerd have nothing to talk to, so the recovery system reboots. We stub `pmset` for now.
- [ ] **Graphics.** A framebuffer driver and then the NVIDIA driver (built on NullMoth's macOS 27 driver) so WindowServer can draw.
- [ ] **USB keyboard and mouse** (HID) in the recovery environment.
- [ ] **Installer app** launching on top of the above.

## Platform
- [ ] **AMD multi-core.** In QEMU the kernel divides by zero while sizing the CPU topology on the EPYC model. The AMD patches avoid it by reporting one core and one thread (`cpus=1`, MSR 0x35 and two topology patches), so real Ryzen runs on one core too. See `docs/notes/amd-patches-review-ee87577.md`.
- [ ] **Real hardware.** First tests on an AMD Ryzen PC: the kernel and launchd boot, WindowServer doesn't (see above). More boards, NVMe and USB wanted, with notes in `hardware-support`.
- [ ] **AMFI.** The AMFI stand-in returns 0 everywhere; a more faithful stub or the real thing would help.
- [ ] **NVRAM and EFI runtime.** Proper runtime services instead of the stub table.
- [ ] **Wi-Fi, audio, Bluetooth.** Not started.

## Tooling
- [x] Rust port of `tools/kcbuild.py`: `crates/anathema-kc`, in main.
- [ ] An installer/setup tool that builds the USB stick (loader, kext collection, root) for you (`crates/anathema-setup`, in progress).
- [ ] Faster test loop: parallel emulator runs, disk snapshots.
