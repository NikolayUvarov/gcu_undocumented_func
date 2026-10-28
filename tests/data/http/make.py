#!/usr/bin/env python3
"""Writes the coded bodies tests/http_coding_host.rs decodes (000-NET-0047): one text (the test builds it again) in gzip (with a name, an extra
field and a comment), zlib and raw DEFLATE; two gzip members; a gzip whose CRC is damaged; and a bomb, 16 MiB of zeros."""
import gzip, io, struct, zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
text = b"".join(b"line %05d: the quick brown fox jumps over the lazy dog %s\n" % (i, bytes([97 + i % 26]) * (i % 40)) for i in range(2000))


def member(data, name=b"body.txt", extra=b"MI\x02\x00ok", comment=b"a test body"):
    # gzip with FEXTRA, FNAME and FCOMMENT (RFC 1952 2.3), which Python's module does not write.
    flags = 4 | 8 | 16
    header = b"\x1f\x8b\x08" + bytes([flags]) + struct.pack("<I", 0) + b"\x00\xff"
    header += struct.pack("<H", len(extra)) + extra + name + b"\x00" + comment + b"\x00"
    body = zlib.compressobj(9, zlib.DEFLATED, -15)
    raw = body.compress(data) + body.flush()
    return header + raw + struct.pack("<II", zlib.crc32(data), len(data) & 0xFFFFFFFF)

(HERE / "body.gz").write_bytes(member(text))
(HERE / "two.gz").write_bytes(gzip.compress(text[:30000], mtime=0) + gzip.compress(text[30000:], mtime=0))
(HERE / "body.zlib").write_bytes(zlib.compress(text, 9))
raw = zlib.compressobj(6, zlib.DEFLATED, -15)
(HERE / "body.deflate").write_bytes(raw.compress(text) + raw.flush())
damaged = bytearray(member(text))
damaged[-6] ^= 0x55
(HERE / "damaged.gz").write_bytes(bytes(damaged))
(HERE / "bomb.gz").write_bytes(gzip.compress(bytes(16 * 1024 * 1024), compresslevel=9, mtime=0))
