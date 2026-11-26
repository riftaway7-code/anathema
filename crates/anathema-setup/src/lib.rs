//! The platform-neutral parts of anathema-setup: the FAT32 writer and the files that go on the
//! stick's EFI partition. The Windows GUI (src/app.rs) and the anathema-esp dev tool use them.

pub mod fat32;
pub mod kernel_files;
pub mod stick;

/// The stubs anathema-xboot installs into the kernel. All three are our own code, not Apple's,
/// built by tools/mkxcp.sh. This one holds the commpage map stubs (tools/commpage_stub.c).
pub const XCP: &[u8] = include_bytes!("../assets/xcp.bin");

/// The commpage enter and fixed locator hooks (tools/commpage_enter.c), which map the commpage
/// into every user task.
pub const XCP2: &[u8] = include_bytes!("../assets/xcp2.bin");

/// Starts tmpfs and the fs compression kexts right before launchd (tools/loadkexts.c).
pub const XCP3: &[u8] = include_bytes!("../assets/xcp3.bin");
