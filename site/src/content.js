// All the words on the page live here, so copy edits never touch layout code.
// Status facts come from the repo README; keep them in step when it changes.

// the repo moves to summythq; github redirects the old riftaway7-code url
export const REPO = "https://github.com/summythq/anathema";
export const DISCORD = "https://discord.gg/ZSQxYC8RT7";
export const FAQ = `${REPO}/blob/main/FAQ.md`;
export const TODO = `${REPO}/blob/main/TODO.md`;
export const SUMMYT = "https://summyt.net";

export const nav = [
  { label: "Status", href: "#status" },
  { label: "How it works", href: "#how" },
  { label: "FAQ", href: FAQ },
];

export const hero = {
  title: "macOS 27 on a regular PC.",
  sub: "Our own UEFI loader boots Apple's real Intel kernel and userspace. No OpenCore. Open source and early.",
  primary: { label: "Join the Discord", href: DISCORD },
  secondary: { label: "View on GitHub", href: REPO },
  image: {
    src: "/boot.png",
    width: 1000,
    height: 410,
    alt: "Verbose boot text from the Darwin 27.2.0 kernel, xnu-13432.41.4 RELEASE_X86_64, on a black screen",
    caption: "The macOS 27.2 Intel kernel starting from an Anathema USB stick, in QEMU.",
  },
};

export const status = {
  title: "Where it stands",
  sub: "Tested in QEMU with USB, SATA and NVMe disks. On a real PC, only the loader bring-up so far.",
  working: [
    { icon: "boot", name: "Own UEFI loader", text: "Starts the macOS 27.2 Intel kernel from a USB stick, with boot text on screen." },
    { icon: "stack", name: "Our kext collection", text: "Apple's Intel drivers for AHCI, NVMe, USB storage, APFS and HFS+, linked by our own tool." },
    { icon: "terminal", name: "Real userspace", text: "Apple's launchd, dyld and the dyld shared cache run on the kernel." },
    { icon: "lifebuoy", name: "Recovery environment", text: "Apple's x86_64 BaseSystem boots from APFS and stays up." },
    { icon: "cpu", name: "Intel and AMD", text: "Haswell to Cascade Lake in QEMU. AMD is detected and patched, one core for now." },
    { icon: "usb", name: "Stick builder", text: "Tools that build the bootable USB image: loader, kernel, drivers and a root partition." },
  ],
  progress: [
    { name: "Windows setup app", text: "Builds the stick for you on Windows. Written, not yet tried on a real Windows PC." },
    { name: "Graphics and the installer window", text: "The NVIDIA driver by NullMoth is being wired in so WindowServer can draw." },
    { name: "Keyboard and mouse", text: "USB HID in the recovery environment." },
    { name: "Real hardware", text: "First boots on real boards, NVMe and USB." },
  ],
  notYet: ["Wi-Fi", "Audio", "Bluetooth", "Sleep", "AMD multi-core", "Real PCs past the loader"],
};

export const how = {
  title: "From power button to launchd",
  chain: [
    { name: "UEFI firmware", text: "Your PC's own firmware loads our loader from the USB stick." },
    { name: "anathema-xboot", text: "Hands the kernel what a Mac's firmware would: memory map, device tree, boot arguments." },
    { name: "macOS 27 kernel", text: "Apple's Intel kernel, xnu-13432, patched in memory, never on disk." },
    { name: "Kext collection", text: "Apple's storage, USB and file system drivers in one block the kernel loads." },
    { name: "Apple userspace", text: "launchd, dyld and the recovery environment, as Apple ships them." },
  ],
  note: "Where a PC lacks something a Mac has, the loader adds a small stand-in: the commpage, a sandbox policy, AMFI and EFI runtime services.",
};

export const help = {
  title: "Help it reach real hardware",
  test: {
    name: "Test it on your PC",
    text: "Boot the stick and post your CPU, board and GPU in hardware-support. A photo of the boot screen helps most.",
  },
  task: {
    name: "Pick a task",
    text: "Sandbox policy, graphics, AMD multi-core and more are open in TODO.md.",
    link: { label: "Open TODO.md", href: TODO },
  },
  report: {
    name: "Report what breaks",
    text: "Post the exact boot text and your hardware in bug-reports.",
  },
};

export const notes = {
  title: "Before you try it",
  items: [
    {
      q: "Is there Apple software in the repo?",
      a: "No. The repo holds only our code. The tools download Apple's kernel and drivers from public kernel debug kits on your own machine.",
    },
    {
      q: "Will it run on my PC today?",
      a: "Probably not yet. It boots in QEMU, and real hardware past the loader is untested. That is exactly where testers help.",
    },
    {
      q: "Who wrote it?",
      a: "Every line was written by an AI (Claude Code) under human direction. It has bugs and has not been audited. Read it as a research log that happens to work.",
    },
    {
      q: "Is this legal?",
      a: "Apple licenses macOS for Apple-branded hardware. Anathema is research into how the platform works, and what you do with it is your responsibility.",
    },
    {
      q: "What about arm64?",
      a: "Anathema started as an arm64 to x86-64 translator for the macOS VM kernel. That work continues in the repo as a research track.",
    },
  ],
};

export const footer = {
  line: "MIT licensed. Not affiliated with Apple. macOS is a trademark of Apple Inc.",
  studio: { label: "A summyt project", href: SUMMYT },
};
