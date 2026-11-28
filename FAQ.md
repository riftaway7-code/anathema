# Anathema FAQ

Short answers for people who just found the project. For the technical details, read the [README](README.md).

* [The basics](#the-basics)
* [Trying it](#trying-it)
* [Hardware](#hardware)
* [Trust, safety and legal](#trust-safety-and-legal)
* [Helping](#helping)

## The basics

### What is Anathema?

A research project that boots **macOS 27** on an ordinary x86-64 PC with **our own UEFI loader** (`anathema-xboot`), without OpenCore. It starts Apple's Intel macOS 27 kernel natively, links the needed Apple kexts (`anathema-kc` / `tools/kcbuild.py`), and brings up Apple's userspace on top.

The repo also keeps the original **arm64 translation** research track (`anathema-xlat`, `anathema-rt`, `anathema-boot`), which translates the `VirtualMac2,1` arm64 VM kernel to x86-64 inside a software virtual machine.

### Does it boot macOS?

**In QEMU, yes—into recovery userspace, not a desktop yet.** On the native Intel path (`anathema-xboot`), Apple's kernel, `launchd`, `dyld`, and the x86_64 recovery environment (`BaseSystem`) boot and stay up in QEMU, though graphics/installer UI and USB HID input are still in progress and real PC hardware beyond early loader bring-up hasn't been tested yet.

On the older arm64 translation track, the kernel boots in the Linux harness up to mounting the RAM disk and stops at `exec` of `/sbin/launchd` (`errno 8`). The [README](README.md#where-it-stands-updated-2026-11-24) is the source of truth for current status.

### Is this a Hackintosh?

Not in the usual OpenCore/Clover sense. Anathema writes its own UEFI loader (`anathema-xboot`) in Rust, links kexts with its own linker (`anathema-kc`), and applies small in-memory patches and stand-ins directly instead of relying on OpenCore. It also explores the arm64 `VirtualMac2,1` translation path as a separate research track.

### Which macOS version?

macOS 27.2 / 27.2.1 (`xnu-13432.41.4`, build `26B131`) for the native Intel loader (using kexts from Tahoe `25G342` and macOS 27 `26B131` KDKs), and the `VirtualMac2,1` (`vma2macosap`) build of macOS 27.2 for the arm64 research track.

### Why is the arm64 track so slow?

In the arm64 translator (`anathema-xlat` / `anathema-rt`), every guest memory access leaves translated code and goes through a software memory manager. That is simple and easy to verify against QEMU, but takes minutes to reach `launchd`. The native Intel path (`anathema-xboot`) runs on the x86-64 CPU directly without instruction translation.

### Is it related to Stela?

No. Stela is a separate project. Anathema looked at it only for ideas, contains none of its code, and is not a fork.

### When will it boot to a desktop?

There is no date. On the native Intel path, the next milestones are wiring in GPU graphics (for WindowServer) and USB keyboard/mouse input so the recovery installer window can run. See [TODO.md](TODO.md).

## Trying it

### Can I try it on my PC?

Not as a finished product yet—right now it is for developers and testers:

* **Windows:** `anathema-setup` builds a USB stick's EFI partition (`anathema-xboot`, `xkernel`, `xkc.bin`, `xcp.bin`) from pinned Apple KDKs, though a full macOS root/installer partition isn't packaged by setup yet.
* **QEMU / Linux / macOS:** the scripts in `tools/` (`xrun.sh`, `xrun_esp.sh`, `rundisk.sh`) and `anathema-esp` build and boot test images in QEMU.
* **Arm64 research track:** `rt-boot` runs on **Linux** and prints the translated arm64 kernel's boot log to your terminal; `anathema-boot` also includes a hardware report, readiness check, and bring-up test.

If you just want to watch progress, see [How to follow along](#how-to-follow-along) below.

### Do I need my own copy of macOS?

Yes, in the sense that the repo contains no Apple binaries or firmware. For the native Intel path, `anathema-setup` and `anathema-media` download Apple's Kernel Debug Kits (from dortania's `KdkSupportPkg` GitHub releases, verified by SHA-256) and `tools/` scripts unpack your own recovery image. For the arm64 `rt-boot` harness, you extract three files from a macOS 27 IPSW you obtained yourself. Please don't share extracted Apple files in issues or in the Discord.

### Can I build it on a Mac?

You can build and run the host-independent unit tests on macOS, Linux, or Windows:

```
cargo test -p anathema-media
cargo test -p anathema-kc
cargo test -p anathema-xlat
```

Building the UEFI loaders (`anathema-xboot`, `anathema-boot`) also works on any host with the `x86_64-unknown-uefi` target (`rust-toolchain.toml` handles this). The arm64 QEMU diff test (`cargo test -p anathema-xlat --test diff`) and `rt-boot fbcheck` require x86-64 Linux with `qemu-aarch64`.

## Hardware

### Will my GPU / Wi-Fi / Ethernet / sound card work?

Not yet:

* **Display / GPU:** Today `anathema-xboot` uses standard UEFI GOP framebuffer graphics for boot text. Integration with NullMoth's macOS 27 NVIDIA driver (RTX 20/30/40/50, separate noncommercial license) is in progress for WindowServer.
* **USB keyboard/mouse, Wi-Fi, audio, Bluetooth:** USB HID input for recovery is in progress; Wi-Fi, audio, and Bluetooth are not started yet.
* *(On the arm64 research track, macOS never sees your real PCIe hardware—it only sees emulated virtual devices.)*

### AMD or NVIDIA?

For display output, UEFI GOP works on any GPU that shows a boot screen, and NVIDIA RTX 20/30/40/50 acceleration is the first GPU driver being wired up. For CPUs, `anathema-xboot` supports Intel CPUs and also patches AMD CPU identification and timing (`cpus=1` in QEMU today; real Ryzen boards still need testing).

### Will iCloud, iMessage or the App Store work?

That idea is parked. Apple services check the machine's identity, and making one up (or borrowing a real Mac's) goes against Apple's terms and can get your Apple account locked. The project won't ship fake or borrowed serial numbers.

### Can it install to my internal SSD?

Not yet—all current test workflows target QEMU images or a USB stick. Don't run experimental boot loaders on a machine that holds data you can't afford to lose without a backup.

## Trust, safety and legal

### The README says it is "fully vibe coded". What does that mean?

Every line was written by an AI (Claude Code) under a human's direction. The maintainer says this up front so nobody mistakes it for audited code. It has bugs.

What keeps it honest:

* unit and integration tests for parsers, the kext linker (`anathema-kc`), FAT32/KDK setup (`anathema-setup`), and the arm64 decoder/emitter (`anathema-xlat`);
* parity checks between `anathema-kc` and `tools/kcbuild.py`, and QEMU diff fuzzing for `anathema-xlat`;
* outside reviews are welcome—open an issue if you find something.

Read it as a research log that happens to compile, not as a product.

### Is it safe to run?

* **The QEMU / Linux test tools** run against disk image files rather than your host OS disks.
* **The `anathema-boot` UEFI loader** only reads internal NVMe drives (its NVMe driver is read-only) and saves bring-up logs to a `AnathemaBringup` UEFI variable (deleted on next boot).
* **`anathema-setup`** (the Windows USB stick tool) **erases the disk you pick**. It allows external USB drives up to 256 GB, not only thumb drives. **Unplug every USB drive except the target stick** before you start, and verify the model and size on the Review screen.

Don't run experimental boot loaders on a machine holding irreplaceable data without a backup.

### Is it legal?

Anathema contains no Apple code, firmware or images. Apple's license for macOS says it may be used on Apple-branded hardware. Anathema is research into how the platform works. What you do with it is your responsibility. This is not legal advice.

### What is the license?

MIT, with one exception: `crates/anathema-rt/src/aesdev.rs` follows the behaviour of a file from QEMU, which is GPL-2.0-or-later, so that file should be treated as GPL-2.0-or-later. See the [README](README.md#credits-and-license).

## Helping

### How can I help without writing code?

* **Hardware tests and reports.** Test on real hardware or share QEMU/boot results in the Discord `hardware-support` channel.
* **Ideas.** Add them to the [ideas repo](https://github.com/riftaway7-code/anathema-ideas). Check the index first, and if your idea is already there, add a +1 instead of a new file.
* **Docs.** If something in the README, this FAQ, or the docs confused you, say so.

### How can I help with code?

* Pick a task from [TODO.md](TODO.md) and mention it in `dev-talk`.
* Run the unit tests (`cargo test -p anathema-media`, `cargo test -p anathema-kc`, `cargo test -p anathema-xlat`) and add more.
* Open an issue before big changes, so work doesn't overlap.

### How do I follow along?

* **Watch the GitHub repo** (Watch → Custom → Releases, or All Activity if you want every commit).
* **Join the Discord** for the weekly devlog and announcements.
* The [README](README.md#where-it-stands-updated-2026-11-24) is the source of truth for how far it boots.
