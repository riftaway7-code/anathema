# Anathema FAQ

Short answers for people who just found the project. For the technical details, read the [README](README.md).

* [The basics](#the-basics)
* [Trying it](#trying-it)
* [Hardware](#hardware)
* [Trust, safety and legal](#trust-safety-and-legal)
* [Helping](#helping)

## The basics

### What is Anathema?

A research project that boots **macOS 27 on an ordinary PC with its own boot loader**. The loader (`anathema-xboot`, a UEFI application on a USB stick) starts Apple's real Intel macOS 27.2 kernel, gives it a kext collection we link ourselves, and brings up Apple's real userspace on top. It doesn't use OpenCore or Clover.

### Does it boot macOS?

**Partly, and only to text so far.**

* In QEMU the kernel, our kext collection, launchd and Apple's recovery environment boot and stay up.
* First tests on a real PC (AMD Ryzen) have started. There the kernel boots and launchd and most daemons start, but WindowServer does not. So there is no installer screen, no desktop and no apps yet.

The [Where it stands section of the README](README.md#where-it-stands) has the exact details and is kept up to date.

### Is this a Hackintosh?

Close, but built differently. A Hackintosh also runs Intel macOS directly on PC hardware, but it uses a community boot loader such as OpenCore. Anathema has its own loader, written for this project. It patches the kernel and fills in what a Mac's firmware would normally provide (device tree, boot arguments, small stand-ins for pieces like AMFI and the Sandbox policy).

### Which macOS version?

macOS 27.2: the Intel kernel `xnu-13432.41.4` (build 26B131), with Intel kexts from the Tahoe and macOS 27 Kernel Debug Kits, and the x86_64 recovery environment (BaseSystem).

### What is the current blocker?

WindowServer, the process that draws the screen. On real hardware, launchd's attempt to start it fails with errno 2 ("no such file or directory"). The suspected cause is that the macOS 27 kernel is running with a userspace from a different build. That is not confirmed yet. After that come the graphics driver, USB keyboard and mouse, and the installer itself. See [TODO.md](TODO.md).

### When will it boot to a desktop?

There is no date. Fix WindowServer first, then graphics, then the installer.

### What happened to the arm64 translation idea?

Anathema started as an attempt to run the *arm64* macOS kernel on a PC by translating its instructions to x86-64. That work is still in the repo as a research track (described in the README). It reaches the point where the kernel tries to start launchd and stops there. The native Intel path is where the progress is.

### Is it related to Stela?

No. Stela is a separate project. Anathema looked at it only for ideas, contains none of its code, and is not a fork.

## Trying it

### Can I try it on my PC?

Only as a tester, not as a normal user. You build a USB stick with the scripts in `tools/` (a setup tool that does this for you on Windows is in progress). The stick boots to boot text on screen and stops short of the installer. Reports from real hardware are very welcome in the Discord `hardware-support` channel.

### Do I need my own copy of macOS?

Not by hand: the repo contains no Apple software, and the tools download what they need. Please don't share Apple files in issues or in the Discord.

### Can I build it on a Mac?

Yes. The loader cross-compiles to UEFI on any machine, including Apple silicon Macs, and the unit tests run there too:

```
cargo build --release -p anathema-xboot --target x86_64-unknown-uefi
cargo test --workspace --exclude anathema-boot --exclude anathema-xboot
```

`tools/xrun_esp.sh` boots a stick image in QEMU on macOS or Linux.

## Hardware

### Which CPUs work?

* **Intel**: Haswell to Cascade Lake, tested in QEMU.
* **AMD**: the loader detects it and patches the kernel's CPU identification and timing. It runs on one core only for now, both in QEMU and on the real PC. Multi-core on AMD is an open task.

### Will my GPU / Wi-Fi / Ethernet / sound card work?

Here macOS talks to your real hardware directly, so every device needs a macOS driver (kext).

* **Storage and USB**: AHCI (SATA), NVMe, USB and USB mass storage are in our kext collection.
* **Graphics**: boot text uses your PC's standard UEFI framebuffer, which works on any card that shows a boot screen. Accelerated graphics is in progress through the NVIDIA driver by NullMoth (RTX 20/30/40/50, separate noncommercial license).
* **Wi-Fi, audio and Bluetooth**: not started.

### Will iCloud, iMessage or the App Store work?

That idea is parked. Apple services check the machine's identity, and making one up (or borrowing a real Mac's) goes against Apple's terms and can get your Apple account locked. The project won't ship fake or borrowed serial numbers.

### Can it install to my internal SSD?

Not yet. Anathema boots from the USB stick. Installing to an internal drive is planned, with strict safety rules. See the [install write-up](https://github.com/riftaway7-code/anathema-ideas/blob/main/writeups/install-to-internal-drive.md).

## Trust, safety and legal

### The README says it is "fully vibe coded". What does that mean?

Every line was written by an AI (Claude Code) under a human's direction. The maintainer says this up front so nobody mistakes it for audited code. It has bugs.

What keeps it honest:

* the loader's kernel patches (nearly all of them) check the bytes they expect before writing, and are skipped if they differ;
* unit tests for the file parsers, the kext collection linker and the setup tool run on any machine;
* outside reviews are welcome. Open an issue if you find something.

Read it as a research log that happens to work, not as a product.

### Is it safe to run?

* The **loader** reads its files from the USB stick and writes a short `boot.log` back to the stick (CPU, framebuffer and memory map details, no personal data).
* Once the kernel is running, **macOS has real storage drivers and can see your internal disks**, just like on a Mac. Nothing in Anathema is meant to write to them, but this is untested research code. Back up or unplug drives you can't afford to lose.
* **anathema-setup** (the Windows tool that prepares the USB stick) **erases the disk you pick**. It also lets you pick external USB SSDs and hard drives up to 256 GB, not only sticks. **Unplug every USB drive except the stick** before you start, and check the model and size on the Review screen.

### Is it legal?

Anathema contains no Apple code, firmware or images. Apple's license for macOS says it may be used on Apple-branded hardware. Anathema is research into how the platform works. What you do with it is your responsibility. This is not legal advice.

### What is the license?

MIT, with one exception: `crates/anathema-rt/src/aesdev.rs` follows the behaviour of a file from QEMU, which is GPL-2.0-or-later, so that file should be treated as GPL-2.0-or-later. Add-ons such as the NullMoth NVIDIA driver keep their own licenses. See the README's "Credits and license" section.

## Helping

### How can I help without writing code?

* **Hardware reports.** Boot the stick on your PC and share your board, CPU and GPU, a photo of the screen where it stops, and the `boot.log` it leaves on the stick.
* **Ideas.** Add them to the [ideas repo](https://github.com/riftaway7-code/anathema-ideas). Check the index first, and if your idea is already there, add a +1 to that file instead of a new one.
* **Docs.** If something in the README or this FAQ confused you, say so. That's a bug too.

### How can I help with code?

* Read the [Where it stands section](README.md#where-it-stands) for the current problem, and pick a task from [TODO.md](TODO.md).
* Run the tests (`cargo test --workspace --exclude anathema-boot --exclude anathema-xboot`) and add more. Parsers and the kext collection linker are good places to start.
* Open an issue before big changes, so work doesn't overlap.

### How do I follow along?

* **Watch the GitHub repo** (Watch → Custom → Releases, or All Activity if you want every commit).
* **Join the Discord** for the weekly devlog and announcements.
* The README's **Where it stands** section is the source of truth for how far it boots.
