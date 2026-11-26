#!/usr/bin/env python3
# links one small x86_64 ELF object (from clang -c -fpie) into a flat blob for anathema-xboot.
# sections named with --at go first, at those offsets, in the order given; every other allocated
# section follows in object order. only pc-relative relocations to sections inside the blob are
# allowed, so the blob runs at whatever address the loader copies it to.
# usage: xcplink.py OBJ OUT [--at SECTION=OFFSET]... [--max BYTES]
import struct, sys

SHF_ALLOC = 0x2
SHT_PROGBITS, SHT_RELA, SHT_NOBITS = 1, 4, 8
R_X86_64_PC32, R_X86_64_PLT32 = 2, 4


def fail(msg):
    sys.exit("xcplink: " + msg)


def main(argv):
    if len(argv) < 2:
        fail("usage: xcplink.py OBJ OUT [--at SECTION=OFFSET]... [--max BYTES]")
    obj, out, rest = argv[0], argv[1], argv[2:]
    fixed, limit = [], None
    while rest:
        flag = rest.pop(0)
        if not rest:
            fail(flag + " needs a value")
        val = rest.pop(0)
        if flag == "--at":
            name, _, off = val.partition("=")
            fixed.append((name, int(off, 0)))
        elif flag == "--max":
            limit = int(val, 0)
        else:
            fail("unknown option " + flag)

    d = open(obj, "rb").read()
    if d[:4] != b"\x7fELF" or d[4] != 2 or d[5] != 1:
        fail(obj + " is not a little-endian elf64 object")
    shoff, = struct.unpack_from("<Q", d, 0x28)
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", d, 0x3a)
    secs = [struct.unpack_from("<IIQQQQIIQQ", d, shoff + i * shentsize) for i in range(shnum)]
    strtab = secs[shstrndx][4]

    def name(i):
        start = strtab + secs[i][0]
        return d[start:d.index(b"\0", start)].decode()

    alloc = [i for i, s in enumerate(secs) if s[2] & SHF_ALLOC and s[5]]
    for i in alloc:
        if secs[i][1] == SHT_NOBITS:
            fail("%s is zero-fill data, which the blob cannot hold" % name(i))
        if secs[i][1] != SHT_PROGBITS:
            fail("%s has section type %d" % (name(i), secs[i][1]))

    # the fixed sections, then the rest
    place, blob = {}, bytearray()
    for want, off in fixed:
        hits = [i for i in alloc if name(i) == want]
        if len(hits) != 1:
            fail("expected one section %s, found %d" % (want, len(hits)))
        if len(blob) > off:
            fail("%s must start at %#x but the blob is already %#x bytes" % (want, off, len(blob)))
        blob += bytes(off - len(blob))
        place[hits[0]] = off
        blob += d[secs[hits[0]][4]:secs[hits[0]][4] + secs[hits[0]][5]]
    for i in alloc:
        if i in place:
            continue
        align = max(secs[i][8], 1)
        blob += bytes(-len(blob) % align)
        place[i] = len(blob)
        blob += d[secs[i][4]:secs[i][4] + secs[i][5]]

    symtabs = [s for s in secs if s[1] == 2]
    syms = []
    if symtabs:
        st = symtabs[0]
        syms = [struct.unpack_from("<IBBHQQ", d, st[4] + k * 24) for k in range(st[5] // 24)]
        symstr = secs[st[6]][4]

    def symname(sym):
        start = symstr + sym[0]
        return d[start:d.index(b"\0", start)].decode() or "(section symbol)"

    for s in secs:
        if s[1] != SHT_RELA or s[7] not in place:
            continue
        target = s[7]
        for k in range(s[5] // 24):
            r_off, info, addend = struct.unpack_from("<QQq", d, s[4] + k * 24)
            kind, sym = info & 0xffffffff, syms[info >> 32]
            if kind not in (R_X86_64_PC32, R_X86_64_PLT32):
                fail("%s: relocation type %d against %s is not pc-relative" % (name(target), kind, symname(sym)))
            if sym[3] not in place:
                fail("%s: %s is not defined inside the blob" % (name(target), symname(sym)))
            p = place[target] + r_off
            v = place[sym[3]] + sym[4] + addend - p
            if not -(1 << 31) <= v < (1 << 31):
                fail("%s: %s is out of range" % (name(target), symname(sym)))
            struct.pack_into("<i", blob, p, v)

    if limit is not None and len(blob) > limit:
        fail("%s is %#x bytes, more than the loader's %#x" % (out, len(blob), limit))
    open(out, "wb").write(blob)
    layout = ", ".join("%s@%#x" % (name(i), place[i]) for i in sorted(place, key=place.get))
    print("%s: %#x bytes (%s)" % (out, len(blob), layout))


main(sys.argv[1:])
