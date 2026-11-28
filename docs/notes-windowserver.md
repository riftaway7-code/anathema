# WindowServer does not start: posix_spawn errno 2 (ENOENT)

Status: desk investigation only (2026-11-28). Nothing was run on hardware or in QEMU, and no Apple
files were opened. The evidence is the repo (loader, `tools/`, `TODO.md`) and the symptom as reported:
on the real PC, launchd and most daemons start, but launchd's `posix_spawn` of WindowServer fails with
errno 2.

## Update 2: decmpfs (2026-11-28)

New fact from riftsclaude's probe of a read-only backup of the stick's APFS root:
`SkyLight.framework/Resources/WindowServer` and `/usr/libexec/xpcproxy` both exist as regular files.
Both are decmpfs-compressed, **type 8**: LZVN, with the compressed data in the resource fork.

- That rules out the original #2 (binary missing).
- #1 (path doesn't resolve) is **not confirmed**: the file is at the expected place, but nobody has
  checked yet that the plist points at that path.
- Both stay below for the record.

### What the repo does with decmpfs and compression

The loader, the kext collection tooling and the patches never handle, patch, stub or skip decmpfs,
`com.apple.decmpfs`, resource forks or `vnode_pager`. I searched for decmpfs, FSCompression,
compression, ResourceFork, rsrc, lzvn, lzfse, zlib and xattr. Every hit is in one of these:

- **`tools/loadkexts.c`** (built into `xcp3.bin`, hooked in by `install_loadkexts`,
  `crates/anathema-xboot/src/main.rs` around line 777: "starts kexts that have no matching personality
  (tmpfs, the fs compression types) ... right before launchd starts"). It calls
  `OSKext::loadKextWithIdentifier` for three kexts:
  - `com.apple.filesystems.tmpfs`
  - `com.apple.AppleFSCompression.AppleFSCompressionTypeZlib`
  - `com.apple.AppleFSCompression.AppleFSCompressionTypeDataless`

  Each result is printed as `anx loadkext <id>: <rc>`. Nothing checks the return code, and nothing
  retries.
- **The kext collection root lists don't name either AppleFSCompression kext.**
  - `tools/kc-roots.txt` (48 ids) has `com.apple.kec.Compression` (the compression library kext, not a
    decmpfs decompressor) and `com.apple.filesystems.tmpfs`, but no `AppleFSCompression*`.
  - The same goes for `tools/mkkc.sh` and `ROOTS` / `KC_ROOTS` in
    `crates/anathema-setup/src/kernel_files.rs:28-43`.
  - Nothing else in the collection depends on them, so they would not be pulled in as dependencies
    (verify).
  - So a collection built from the repo's lists probably has **no decmpfs decompressor**, and the
    `loadkexts` calls for them would fail.
  - The stick, however, boots `xkc_full.bin` (`tools/finalize.sh:6`), which is built outside the repo.
    Whether it contains them is unknown (verify).
- `crates/anathema-media/src/apfs.rs:586-651`: the repo's own read-only APFS reader. It decodes
  decmpfs types 3, 4, 7, 8, 11, 12, 13 and 14 from `com.apple.decmpfs` and `com.apple.ResourceFork`.
  It is host tooling, not involved at boot, but it can extract these files from the backup (see the
  checks below).
- `crates/anathema-media/src/kdk/hfs.rs:197` refuses decmpfs files in the KDK's HFS+ image. That is
  host tooling and irrelevant here.

No byte patch in `EARLY_VM_PATCHES`, `NOKEXT_PATCHES` or `AMD_PATCHES` names decmpfs, the vnode
pager or APFS. The early-VM patches ("early vm stub", "early pv repair", kernel map range bounds) do
touch the VM layer that page-ins go through, so an indirect effect on page-in can't be excluded from
the repo alone. That is low likelihood (verify).

### Why other daemons still run

If xpcproxy is type 8 and daemons run, then type 8 decompression works on this boot:

- On current macOS, launchd starts each daemon by spawning `/usr/libexec/xpcproxy`. xpcproxy then
  execs the job's program (`posix_spawn` with `POSIX_SPAWN_SETEXEC`). So xpcproxy itself is read and
  executed for every daemon that runs (verify for this build, for example in launchd's log or `ps`
  output from `diag.sh`).
- So the AppleFSCompression decompressor must be loaded and registered on the stick (via
  `xkc_full.bin` plus the `loadkexts` hook). That argues **against** "type 8 can't be decompressed at
  all".
- launchd, syslogd, bash and dyld are very likely compressed the same way, since Apple compresses
  nearly all system binaries (verify: list their decmpfs types).

What could still differ for WindowServer:

- **Its file actions or working directory**, not its binary (new hypothesis A below).
- **A failure in that one file's resource fork.** It could be a stream-backed (large) fork where
  xpcproxy's is embedded, or a damaged fork (hypothesis B).
- **Timing.** If WindowServer is started before `loadkexts` has registered the decompressor, its read
  would fail while later ones succeed (hypothesis C). `tools/startws.sh` waits 45 s before kickstarting,
  which suggests timing was already a suspect.

### Where errno 2 could come from (ENOENT, given the binary exists)

- **A path in the spawn request other than the executable.** `posix_spawn` also fails with ENOENT when
  a file action can't open its path (`StandardOutPath` / `StandardErrorPath` in a directory that
  doesn't exist) or a chdir/chroot target is missing (`WorkingDirectory`, `RootDirectory`). Whether
  launchd passes these as spawn file actions or opens them itself in this build is **verify**.
- **The error comes from xpcproxy's exec, not launchd's spawn.** If so, the "posix_spawn errno 2" in
  launchd's log is xpcproxy reporting that `posix_spawn(SETEXEC)` of the WindowServer path failed
  (verify against the exact log line).
- **A decmpfs lookup that fails.** decmpfs gets the compressed data with a getxattr of
  `com.apple.ResourceFork`. A missing attribute is normally `ENOATTR` (93), not `ENOENT` (2), so a
  failed resource fork lookup would have to be mapped to ENOENT somewhere in APFS or decmpfs to give
  this errno (verify in XNU's `decmpfs.c` / APFS behaviour). Also, when no decompressor is registered
  for a type, XNU's decmpfs treats or reports the file in a way I can't pin down from memory: it may
  fall back to the empty data fork (which gives `ENOEXEC`-like failures), or return `ENOTSUP` (verify).

### New ranking

1. **A. ENOENT comes from a non-executable path in the job** (likely; verify).
   - `WorkingDirectory`, `RootDirectory`, `StandardOutPath` / `StandardErrorPath`, or the `Program`
     path itself differing from where the file is.
   - Fits the facts best: the binary exists, decompression works for xpcproxy, and ENOENT literally
     means a missing path.
   - The `_Conclave` strip (old #3) may matter here: a conclave job may expect directories that only
     exist in its conclave view.
2. **B. WindowServer's own decmpfs data can't be read at exec time** (possible; verify).
   - Its resource fork is damaged, or stored differently from xpcproxy's (stream vs embedded), or it
     uses a chunk layout the kernel path rejects.
   - This needs the decmpfs/APFS error to surface as ENOENT, which is unusual.
   - The extraction check below settles it.
3. **C. The decompressor isn't registered yet when WindowServer starts, or `loadkexts` failed** (less
   likely).
   - Daemons that run through type-8 xpcproxy show it is registered at some point. WindowServer would
     have to be spawned before that.
   - The `anx loadkext ...: <rc>` lines in the serial log settle it.
   - Related repo gap, worth fixing either way: the repo's kext root lists don't include
     `AppleFSCompressionTypeZlib` / `TypeDataless`. A stick built by `anathema-setup` probably can't
     decompress anything.
4. **D. Old #1: kernel/userspace build mismatch behind a path difference.** Not confirmed. It folds
   into A if the plist's `Program` differs from where the file actually is.
5. **E. Early-VM patches breaking page-in of compressed files.** Low; no direct evidence. And ENOENT
   is an odd result for a page-in failure (that would more likely be `EIO` or a crash).

### Cheapest checks (no hardware)

All of these work on the read-only APFS backup with the repo's own tool
(`cargo run -p anathema-media -- <cmd>`; its `fsls` and `extract` commands read decmpfs):

1. **Decompress WindowServer**:
   `anathema-media extract <backup> /System/Library/PrivateFrameworks/SkyLight.framework/Resources/WindowServer --out ws.bin`.
   - Check that it succeeds and the size matches the decmpfs header.
   - Check `ws.bin` is an x86_64 Mach-O whose `LC_LOAD_DYLINKER` is `/usr/lib/dyld`.
   - Do the same for xpcproxy and compare. If WindowServer fails to decompress and xpcproxy doesn't,
     B moves to the top.
2. **Compare decmpfs types and fork storage of binaries that do run**: launchd, syslogd, xpcproxy,
   bash and one or two other running daemons, against WindowServer. Look at the type, the fork size,
   and whether the fork is embedded or stream-backed. `apfs.rs`'s `xattr()` already distinguishes the
   two, so a one-line debug print is enough.
3. **Read the patched `com.apple.WindowServer.plist`** from the backup. Note `Program`,
   `ProgramArguments`, `WorkingDirectory`, `RootDirectory`, `StandardOutPath`, `StandardErrorPath` and
   any underscore keys left after the `_Conclave` strip. `fsls` every path it names, or its parent
   directory. Any one that's missing points to A.
4. **Grep the serial log of an existing boot** (rift's or `finalize.sh`'s `/tmp/fin_clean.log`) for
   `anx loadkext`, and for the exact launchd line with the errno: does it name WindowServer's path, or
   xpcproxy?
5. **Check `xkc_full.bin` for the bundle ids**: `grep -c AppleFSCompressionTypeZlib xkc_full.bin`.
   Then check the Tahoe KDK's `AppleFSCompressionTypeZlib.kext` Info.plist for which types it says it
   provides (it should list type 8; verify).

The original analysis follows unchanged, except that each hypothesis heading now has its status.

## What errno 2 from posix_spawn narrows it to

`posix_spawn` returns an error only for problems that come up while the kernel is setting up the exec.
Anything that goes wrong later, inside dyld or in WindowServer's own startup, shows up as a crash or
exit status, not as a spawn errno. In XNU, `ENOENT` at this stage means one of:

- **Path lookup failed.** The executable path, or a directory or symlink on the way to it, does not
  resolve on the mounted root.
- **The dynamic linker named by the binary's `LC_LOAD_DYLINKER` was not found.** `load_dylinker`
  returns `LOAD_ENOENT`, which becomes `ENOENT`.

That rules a few things down straight away:

- A missing dyld shared cache entry makes dyld abort after the exec, so launchd would log a crash or
  exit, not errno 2.
- Code signing and trust cache failures give other errors or kill the process (`EBADEXEC`, a
  `CODESIGNING` kill, `EPERM`), not ENOENT.
- dyld itself is clearly present: launchd, syslogd and the other daemons run through it.

So the question is: **which path does launchd hand the kernel for WindowServer, and why doesn't it
resolve on this root?**

## Hypotheses, most likely first

### 1. The executable path doesn't resolve on the booted root, because of a userspace/kernel build mismatch (not confirmed: the binary exists, see update 2)

Evidence that kernel and userspace come from different builds:

- The kernel is 27.2, build 26B131 (README; `crates/anathema-media/src/kdk/pin.rs`, `KDK_27`).
- The kexts are linked "Tahoe kexts first, the 27 KDK preferred for the kexts that must match"
  (`crates/anathema-setup/src/kernel_files.rs:51`). The Tahoe KDK is 26.7.3, build 25G342 (`pin.rs`,
  `KDK_TAHOE`).
- The loader runs with **Tahoe's AMFI** and stubs the 27 interface:
  - `crates/anathema-xboot/src/main.rs:243`: "skip trust_cache_runtime_init (needs the 27 amfi
    interface, absent with Tahoe's amfi)"
  - the comment above `install_amfi_standin`: "tahoe's amfi can't serve the 27 kernel's entitlement
    bridge"
- The helper root is built from **Tahoe's x86_64 software-update ramdisk**:
  `tools/fixlinks.py:2` reads `~/tahoe/su/AssetData/usr/standalone/update/ramdisk/x86_64SURamDisk.dmg`
  (commit bff31aa, "build a real hfs+ root from the x86_64 ramdisk").
- The APFS recovery root (`/tmp/full.img` partition 4 in `tools/finalize.sh`) is built outside the
  repo, and **nothing in the repo records its build**. Given the above, it is most likely a Tahoe-era
  x86_64 BaseSystem. That is an inference, not checked.

Why a mismatch would show up as ENOENT for WindowServer only: WindowServer is the one job the repo
treats specially. Its launchd plist carries `_Conclave`, which launchd rejects without a trust cache,
so the patcher deletes the key (see hypothesis 3). If the plist's `Program` / `ProgramArguments`
points somewhere that only exists in the build the plist was written for, the lookup fails. That would
be a path inside a cryptex, a conclave image or a graft, or a SkyLight location that moved between
releases. Every other daemon uses plain paths, so it starts. This mechanism is a guess: no plist
contents are in the repo.

How to confirm (no hardware needed, on the built image):

1. `plutil -p` `System/Library/LaunchDaemons/com.apple.WindowServer.plist` on the APFS root of
   `final.img`: note `Program`, `ProgramArguments[0]`, and any other underscore keys besides
   `_Conclave`.
2. `ls -l` that path and every symlink on the way. For SkyLight, `Resources` and `Versions/Current`
   are both symlinks.
3. `otool -l` the WindowServer binary and check `LC_LOAD_DYLINKER`. It should be `/usr/lib/dyld`.
4. Write down the BaseSystem build (`System/Library/CoreServices/SystemVersion.plist`) next to the
   kernel's 26B131.

### 2. WindowServer binary missing from the root (ruled out: the file exists, see update 2)

If the root that boots is closer to an update or restore ramdisk than to a full recovery BaseSystem,
there may be no WindowServer binary at all. An update ramdisk has no GUI. Evidence: the repo does use
the SU ramdisk for the helper root (`tools/fixlinks.py`, `tools/mkroot.sh`). `tools/mkdisk4.sh` and
`tools/finalize.sh` say the final stick boots the APFS recovery container, not the helper. Step 2
above settles it.

### 3. The `_Conclave` strip (contributing, unlikely to be the direct cause)

`tools/patchroot.sh:23-30` and `tools/patchroot2.sh:20-26` run
`sed -e '/_Conclave/{N;d;}'` on `com.apple.WindowServer.plist` (and `com.apple.audiomxd.plist`). That
deletes the key line and **one** following line.

- If the value is a one-line `<string>`/`<true/>`, the plist stays valid. If it is a multi-line `<dict>`
  or `<array>`, the result is broken XML and launchd would refuse the job. That case gives "service
  not found", not an ENOENT spawn. A spawn is being attempted, so the strip probably left a valid
  plist.
- The real risk is the meaning of the key. A `_Conclave` service is presumably meant to be spawned into
  a conclave with its own view of the filesystem. Stripping the key makes launchd spawn it as a normal
  process, with whatever paths the plist names. This is how #3 feeds into #1.
- If the plist is binary rather than XML, `sed` does nothing. Then launchd would reject the job
  outright, which again isn't ENOENT.

Check: diff the patched plist against the original, and confirm it parses with `plutil -lint`.

### 4. The loader's disk / mount layout on real hardware (contributes to later failures, unlikely for this errno)

`tools/mountpayload.sh` mounts the NVIDIA payload partition by trying a fixed list:
`disk0s1-3, disk1s1, disk2s1-3, disk3s1-3`.

- `mkdisk4.sh` puts the payload at **partition 2** on the final stick (ESP p1, payload p2, APFS p3).
- In QEMU the stick is the only disk (`disk0`), so `disk0s2` works.
- On a real PC with internal drives, the stick is usually `disk1` or higher, and the APFS container
  adds a synthesized disk. **`disk1s2` and anything at `disk4` or above are not in the list**, so the
  payload likely never mounts.
- Then `/Library/GPUBundles`, `/Library/NullMoth/nvfw` and `/usr/share/nvfw` (symlinks made in
  `patchroot2.sh:29-33`) dangle.

That breaks the GPU driver, and it is a real-hardware-only difference. But it gives ENOENT at spawn
only if WindowServer's executable path or its dynamic linker goes through the payload, and nothing in
the repo does that. Expect this to be the *next* failure once WindowServer spawns. The script should
find the partition by its `.anathema-payload` marker on any `diskNsM`, or by GPT partition name.

### 5. Trust cache / AMFI stand-in (unlikely for ENOENT)

`trust_cache_runtime_init` is skipped (`main.rs:243`), and the AMFI stand-in returns 0 for every call
("no entitlement present and nothing denied"). Those failure modes are code-signing kills or
entitlement denials, not path errors. They are still likely to matter right after WindowServer
spawns, because WindowServer checks entitlements on its Mach services.

### 6. Missing dyld shared cache entries (ruled out for this symptom)

The shared cache works for the other daemons (README: "the real dyld shared cache run on the kernel").
A missing SkyLight or other framework image would make dyld abort after a successful spawn, so the
error wouldn't be errno 2.

## Gaps in the repo that block a firmer answer

- `diag.sh` and `com.anathema.diag.plist` are copied by `patchroot2.sh:42-45` and read by
  `finalize.sh` ("diag: start"), but **neither file is in the repo**.
- `tools/startws.sh` (sleep 45, then `launchctl kickstart -k system/com.apple.WindowServer`) is not
  installed by any script in the repo.
- There is no record of the BaseSystem build or of how `/tmp/full.img`, `xkc_full.bin` and
  `payload_hfs.img` were made. `tools/kc-roots.txt` has no IOSurface, IOGraphics or IOAccelerator
  kexts. `xkc_full.bin` may add them, but the repo can't say.
- **Nothing in the repo shows WindowServer ever starting in QEMU either.** The `finalize.sh` check only
  greps for launchd, BaseSystem and panic lines. If the QEMU final-image boot shows the same errno 2,
  this is not a real-hardware problem, and #4 drops out for this error.

## Suggested next steps (cheapest first)

1. On the built `final.img` (any Mac or Linux box, read-only): run the four checks under #1, plus
   `plutil -lint` on the patched plist. This most likely settles #1, #2 and #3 without booting anything.
2. Boot `final.img` in QEMU with `tools/rundisk2.sh` and a kickstart of WindowServer, and grep the
   serial log for the same `posix_spawn` error. That separates "always broken" from "real hardware
   only".
3. Commit `diag.sh`, `com.anathema.diag.plist`, and a note of the BaseSystem build, so the root can be
   rebuilt.
4. Separately, make `mountpayload.sh` find the payload partition by its marker on any disk (#4).
