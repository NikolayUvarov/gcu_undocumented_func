# Counts the A64 instruction classes in the executable segments of aarch64 ELF files (215-PRT-0016): what the
# emulator must run. Usage: python3 scripts/a64_census.py aarch64_root/*.elf
import struct, sys, pathlib, collections

def text_words(path):
    data = path.read_bytes()
    if data[:4] != b"\x7fELF" or data[4] != 2 or data[5] != 1 or struct.unpack_from("<H", data, 18)[0] != 183:
        return None  # not ELF64 little-endian aarch64
    phoff, = struct.unpack_from("<Q", data, 32)
    phentsize, phnum = struct.unpack_from("<HH", data, 54)
    for i in range(phnum):
        p_type, p_flags, p_offset, _, _, p_filesz = struct.unpack_from("<IIQQQQ", data, phoff + i * phentsize)
        if p_type == 1 and p_flags & 1:  # PT_LOAD, executable
            seg = data[p_offset:p_offset + p_filesz]
            for j in range(0, len(seg) - 3, 4):
                yield struct.unpack_from("<I", seg, j)[0]

def classify(w):
    op0 = (w >> 25) & 0xF
    if w == 0:
        return "zero word"
    if op0 & 0b1110 == 0b1000:
        return "data processing, immediate"
    if op0 & 0b1110 == 0b1010:
        if w & 0xFFE0001F == 0xD4000001: return "svc"
        if w & 0xFFE0001F == 0xD4200000: return "brk"
        if w & 0xFFFFF01F == 0xD503201F:
            hint = (w >> 5) & 0x7F
            return {0: "nop", 1: "yield", 2: "wfe", 3: "wfi", 4: "sev"}.get(hint, f"hint {hint}")
        if w & 0xFFFFF0FF == 0xD503309F: return "dsb"
        if w & 0xFFFFF0FF == 0xD50330BF: return "dmb"
        if w & 0xFFFFF0FF == 0xD50330DF: return "isb"
        if w & 0xFFF00000 == 0xD5300000: return f"mrs s{(w>>19)&3}_{(w>>16)&7}_c{(w>>12)&15}_c{(w>>8)&15}_{(w>>5)&7}"
        if w & 0xFFF00000 == 0xD5100000: return f"msr s{(w>>19)&3}_{(w>>16)&7}_c{(w>>12)&15}_c{(w>>8)&15}_{(w>>5)&7}"
        if w & 0xFFF80000 == 0xD5080000: return "sys (dc/ic/at/tlbi)"
        if w & 0xFFF8F01F == 0xD500401F: return "msr immediate (pstate)"
        return "branch"
    if op0 & 0b0101 == 0b0100:
        if (w >> 26) & 1:
            return "FP/SIMD load or store"
        if w & 0x3F000000 == 0x08000000:
            return "exclusive or ordered load/store (ldxr/stxr/ldar/stlr/cas)"
        if w & 0x3B200C00 == 0x38200000:
            return "LSE atomic (ldadd/swp/...)"
        return "integer load or store"
    if op0 & 0b0111 == 0b0101:
        return "data processing, register"
    if op0 & 0b0111 == 0b0111:
        return "FP/SIMD data processing"
    return f"unallocated or SVE/SME (op0={op0:04b})"

def main():
    total, files, per_class, where = collections.Counter(), 0, collections.Counter(), collections.defaultdict(collections.Counter)
    for path in sorted(pathlib.Path(a) for a in sys.argv[1:]):
        words = text_words(path)
        if words is None:
            continue
        files += 1
        for w in words:
            c = classify(w)
            per_class[c] += 1
            where[c][path.name] += 1
    n = sum(per_class.values())
    print(f"{files} aarch64 ELF files, {n} words in executable segments")
    for c, k in per_class.most_common():
        top = ", ".join(f"{f} {m}" for f, m in where[c].most_common(4))
        print(f"{k:>10} {100*k/n:6.2f}%  {c}" + (f"   [{top}]" if k < n * 0.01 else ""))

main()
