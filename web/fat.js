// The boot volume's files, read as the loader reads them (215-PRT-0017): FAT12 or FAT16, the root directory, long names.
// The page needs only the programs of BOOT_FILES from it; the kernel's vfs_server reads the rest itself.

/// The files in the volume's root directory: a Map of lower-case long names to their bytes.
export function rootFiles(volume) {
    const view = new DataView(volume.buffer, volume.byteOffset, volume.byteLength);
    const sector = view.getUint16(11, true), perCluster = view.getUint8(13), reserved = view.getUint16(14, true);
    const fats = view.getUint8(16), rootEntries = view.getUint16(17, true);
    const total = view.getUint16(19, true) || view.getUint32(32, true), perFat = view.getUint16(22, true);
    if (sector !== 512 || perFat === 0) throw new Error('the boot volume is not FAT12 or FAT16');
    const fat = reserved * sector;
    const root = fat + fats * perFat * sector;
    const data = root + rootEntries * 32;
    const clusters = Math.floor((total * sector - data) / (perCluster * sector));
    const fat12 = clusters < 4085;
    const next = (cluster) => {
        if (!fat12) return view.getUint16(fat + cluster * 2, true);
        const word = view.getUint16(fat + Math.floor(cluster * 3 / 2), true);
        return cluster & 1 ? word >> 4 : word & 0xFFF;
    };
    const end = fat12 ? 0xFF8 : 0xFFF8;
    const read = (first, size) => {
        const out = new Uint8Array(size), bytes = perCluster * sector;
        for (let cluster = first, at = 0; at < size && cluster >= 2 && cluster < end; cluster = next(cluster), at += bytes) {
            const from = data + (cluster - 2) * bytes;
            out.set(volume.subarray(from, from + Math.min(bytes, size - at)), at);
        }
        return out;
    };
    const files = new Map();
    let long = [];
    for (let i = 0; i < rootEntries; i++) {
        const at = root + i * 32, first = volume[at], attributes = volume[at + 11];
        if (first === 0) break;
        if (first === 0xE5) { long = []; continue; }
        if (attributes === 0x0F) {
            // A long-name part: 13 UTF-16 units in three runs, parts stored last first.
            const units = [];
            for (const [from, count] of [[1, 5], [14, 6], [28, 2]]) for (let k = 0; k < count; k++) units.push(view.getUint16(at + from + 2 * k, true));
            long.unshift(units);
            continue;
        }
        if (attributes & 0x18) { long = []; continue; } // a directory or the volume's label
        const short = () => {
            const base = String.fromCharCode(...volume.subarray(at, at + 8)).trimEnd(), ext = String.fromCharCode(...volume.subarray(at + 8, at + 11)).trimEnd();
            return ext ? `${base}.${ext}` : base;
        };
        const units = long.flat();
        const stop = units.findIndex((u) => u === 0 || u === 0xFFFF);
        const name = long.length ? String.fromCharCode(...(stop < 0 ? units : units.slice(0, stop))) : short();
        files.set(name.toLowerCase(), read(view.getUint16(at + 26, true), view.getUint32(at + 28, true)));
        long = [];
    }
    return files;
}
