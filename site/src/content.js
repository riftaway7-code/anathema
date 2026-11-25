// All the words on the page live here, so copy edits never touch layout code.
// Status facts come from the repo README; keep them in step when it changes.

// the live repo. switch to https://github.com/summythq/anathema once the transfer is done
// (github then redirects the old url to the new one, never the other way round).
export const REPO = "https://github.com/riftaway7-code/anathema";
export const DISCORD = "https://discord.gg/ZSQxYC8RT7";
export const TODO = `${REPO}/blob/main/TODO.md`;
export const CONTRIBUTING = `${REPO}/blob/main/CONTRIBUTING.md#working-on-the-code`;
export const README = `${REPO}#readme`;
export const SUMMYT = "https://summyt.net";

export const nav = [
  { label: "How it works", href: "#how" },
  { label: "Status", href: "#status" },
  { label: "FAQ", href: "#faq" },
  { label: "Get involved", href: "#help" },
];

export const cta = {
  discord: { label: "Join the Discord", href: DISCORD },
  github: { label: "Star on GitHub", href: REPO },
};

export const hero = {
  eyebrow: "Early research. Runs in QEMU today.",
  title: "macOS 27 on a regular PC.",
  sub: "Anathema is our own UEFI loader. It boots Apple's real Intel kernel and userspace, with no OpenCore.",
};

// the hero render (site/render/monitor.py). the monitor is a studio render; the screen
// shows real qemu captures of anathema booting.
export const stage = {
  frames: 40,
  large: "/frames/1440",
  small: "/frames/800",
  width: 1440,
  height: 810,
  bootOff: "/frames/1440/boot_off.webp",
  bootLoader: "/frames/1440/boot_loader.webp",
  alt: "A PC monitor on a stand, its screen showing the macOS 27.2 Intel kernel's verbose boot text",
  caption: "Studio render. The screen shows real captures of Anathema booting in QEMU.",
};

// both are real captures from qemu's display, 1280x720, booting an anathema stick
export const screens = {
  kernel: {
    src: "/boot.webp",
    alt: "Verbose boot text from the Darwin 27.2.0 kernel, xnu-13432.41.4 RELEASE_X86_64, on a black screen",
  },
  loader: {
    src: "/loader.webp",
    alt: "UEFI boot text: the firmware starts the USB stick, then anathema-xboot reports the kernel's segments, entry point and boot data",
  },
};

// read one line at a time as the visitor scrolls
export const statement = ["Apple's own kernel.", "Apple's own drivers.", "Apple's own launchd.", "On a PC Apple never made."];

export const how = {
  title: "From power button to launchd",
  chain: [
    { name: "UEFI firmware", text: "Your PC's own firmware loads our loader from the USB stick.", screen: "loader" },
    { name: "anathema-xboot", text: "Hands the kernel what a Mac's firmware would: memory map, device tree, boot arguments.", screen: "loader", ours: true },
    { name: "macOS 27 kernel", text: "Apple's Intel kernel, patched in memory by the loader.", screen: "kernel" },
    { name: "Kext collection", text: "Apple's storage, USB and file system drivers in one block the kernel loads.", screen: "kernel" },
    { name: "Apple userspace", text: "launchd, dyld and the recovery environment, as Apple ships them.", screen: "kernel" },
  ],
  note: "Where the kernel expects something only a Mac provides, the loader adds a small stand-in: a commpage hook, a sandbox policy, AMFI and EFI runtime services.",
};

export const status = {
  title: "Where it stands",
  updated: "Updated 24 Nov 2026",
  sub: "Tested in QEMU with USB, SATA and NVMe disks. On a real PC, only the loader bring-up so far. Every line was written by an AI (Claude Code) under human direction, and none of it has been audited.",
  labels: { works: "Works", progress: "In progress", none: "Not yet" },
  rows: [
    { state: "works", name: "Own UEFI loader", text: "Starts the macOS 27.2 Intel kernel from a USB stick, with boot text on screen." },
    { state: "works", name: "Kext collection", text: "AHCI, NVMe, USB storage, APFS and HFS+, linked by our own tool." },
    { state: "works", name: "Real userspace", text: "Apple's launchd, dyld and the dyld shared cache run on the kernel." },
    { state: "works", name: "Recovery environment", text: "Apple's x86_64 BaseSystem boots from APFS and stays up." },
    { state: "works", name: "Intel and AMD", text: "Haswell to Cascade Lake in QEMU. AMD is detected and patched, one core for now." },
    { state: "works", name: "Stick builder", text: "Tools that build the bootable USB image: loader, kernel, drivers and a root partition." },
    { state: "progress", name: "Graphics", text: "The NVIDIA driver by NullMoth is being wired in so WindowServer and the installer can draw." },
    { state: "progress", name: "Windows setup app", text: "Builds the stick for you. In the repo, not yet tried on a real Windows PC." },
    { state: "progress", name: "Keyboard and mouse", text: "USB HID in the recovery environment." },
    { state: "progress", name: "Real hardware", text: "First boots on real boards, NVMe and USB." },
  ],
  none: ["Wi-Fi", "Audio", "Bluetooth", "Sleep", "AMD multi-core", "Real PCs past the loader"],
};

export const notes = {
  title: "Before you try it",
  lede: { text: "The full status is in the", link: { label: "README", href: README } },
  items: [
    {
      q: "What hardware does it target?",
      a: "x86-64 PCs that boot UEFI from USB. Intel CPUs (Haswell to Cascade Lake tested in QEMU) and AMD (detected and patched, one core for now). NVIDIA graphics through NullMoth's driver is in progress. No Wi-Fi, audio or Bluetooth yet.",
    },
    {
      q: "How do I try it?",
      a: "There is no one-click download yet. Build the stick with the scripts in tools/ (see CONTRIBUTING.md), or boot it in QEMU first. Ask in #hardware-support before you boot a real PC.",
    },
    {
      q: "Is it safe for my PC?",
      a: "Use a spare USB stick, since building it erases the stick. This is untested research software, so back up anything you care about first.",
    },
    {
      q: "Is there Apple software in the repo?",
      a: "No. The repo holds only Anathema's code. The setup tool downloads Apple's kernel and drivers from public Kernel Debug Kits, on your own machine.",
    },
    {
      q: "Is this a Hackintosh?",
      a: "In a sense. It runs Intel macOS on a PC, but with its own UEFI loader instead of OpenCore, and a kext collection it builds itself from Apple's Kernel Debug Kits.",
    },
    {
      q: "Who wrote it?",
      a: "Every line was written by an AI (Claude Code) under human direction. It has bugs and has not been audited. Read it as a research log that happens to work.",
    },
    {
      q: "Is this legal?",
      a: "Apple licenses macOS for Apple-branded hardware. Anathema is research into how the platform works, and what you do with it is your responsibility.",
    },
  ],
};

export const help = {
  title: "Help it reach real hardware.",
  sub: "Boot the stick on your PC and post your CPU, board and GPU. A photo of the boot screen helps most.",
  channel: "#hardware-support",
  items: [
    { name: "Build the stick", text: "No download yet. The scripts in tools/ build it.", link: { label: "Read CONTRIBUTING.md", href: CONTRIBUTING } },
    { name: "Report what breaks", text: "Post the exact boot text and your hardware.", channel: "#bug-reports", link: { label: "Open the Discord", href: DISCORD } },
    { name: "Pick a task", text: "Sandbox policy, graphics and AMD multi-core are open.", link: { label: "Open TODO.md", href: TODO } },
  ],
};

export const footer = {
  line: "MIT licensed, with exceptions listed in the README. Not affiliated with Apple. macOS is a trademark of Apple Inc.",
  links: [
    { label: "GitHub", href: REPO },
    { label: "Discord", href: DISCORD },
    { label: "README", href: README },
  ],
  studio: { label: "A summyt project", href: SUMMYT },
};
