// The kernel's screen (BGRX) as a PNG file, for a run in Node (215-PRT-0017): RGB, 8 bits, no filter, deflated.
import { deflateSync, crc32 } from 'node:zlib';

function chunk(type, data) {
    const out = Buffer.alloc(12 + data.length);
    out.writeUInt32BE(data.length, 0);
    out.write(type, 4, 'ascii');
    data.copy(out, 8);
    out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
    return out;
}

export function png(bgrx, width, height) {
    const rows = Buffer.alloc((width * 3 + 1) * height);
    for (let y = 0; y < height; y++) {
        const row = y * (width * 3 + 1);
        for (let x = 0; x < width; x++) {
            const from = (y * width + x) * 4, to = row + 1 + x * 3;
            rows[to] = bgrx[from + 2]; rows[to + 1] = bgrx[from + 1]; rows[to + 2] = bgrx[from];
        }
    }
    const header = Buffer.alloc(13);
    header.writeUInt32BE(width, 0); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 2;
    return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header), chunk('IDAT', deflateSync(rows)), chunk('IEND', Buffer.alloc(0))]);
}
