#!/usr/bin/env python3
"""scripts/media.py (000-KRN-0093): its BMP and AVI readers and its PNG writer on small files made here, the AVI laid
out as libmind/src/avi.rs writes it. With ffmpeg on the machine, the three commands on those files too."""
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib
from fractions import Fraction
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import media  # noqa: E402


def bmp(width, height, pixels, bits=24, top_down=False):
    """A BMP of `pixels` (rows of 0xRRGGBB from the top), as `screenshot` writes it at 24 bits (shell/src/bmp.rs)."""
    step, stride = bits // 8, (width * bits // 8 + 3) & ~3
    rows = pixels if top_down else pixels[::-1]
    body = b"".join(b"".join(struct.pack("<I", p)[:step] for p in row).ljust(stride, b"\0") for row in rows)
    header = struct.pack("<2sIHHI", b"BM", 54 + len(body), 0, 0, 54)
    info = struct.pack("<IiiHHIIiiII", 40, width, -height if top_down else height, 1, bits, 0, len(body), 2835, 2835, 0, 0)
    return header + info + body


def avi(width, height, fps, frames, final=True):
    """An AVI as libmind/src/avi.rs writes it: a 224-byte header, `00dc` chunks (no bytes: the picture before again),
    `idx1`. Without `final` the header keeps the counts of an empty file, as when a recording stops before rewriting it."""
    movi, index, largest = b"", b"", 0
    for frame in frames:
        index += b"00dc" + struct.pack("<III", 0x10 if frame else 0, len(movi) + 4, len(frame))
        movi += b"00dc" + struct.pack("<I", len(frame)) + frame + b"\0" * (len(frame) & 1)
        largest = max(largest, len(frame))
    counted, count = (movi, len(frames)) if final else (b"", 0)
    idx1 = b"idx1" + struct.pack("<I", len(index)) + index if final else b""
    words = lambda *w: struct.pack(f"<{len(w)}I", *w)  # noqa: E731
    head = (b"RIFF" + words(216 + len(counted) + len(idx1)) + b"AVI LIST" + words(192) + b"hdrlavih"
            + words(56, 1_000_000 // fps, largest * fps, 0, 0x10 if final else 0, count, 0, 1, largest, width, height, 0, 0, 0, 0)
            + b"LIST" + words(116) + b"strlstrh" + words(56) + b"vidsMJPG" + words(0, 0, 0, 1, fps, 0, count, largest, 0xFFFFFFFF, 0)
            + bytes(4) + struct.pack("<HH", width, height) + b"strf" + words(40, 40, width, height, 1 | 24 << 16) + b"MJPG"
            + words(width * height * 3, 0, 0, 0, 0) + b"LIST" + words(4 + len(counted)) + b"movi")
    assert len(head) == 224
    return head + movi + idx1


def png_pixels(data):
    """Width, height and RGB rows of an unfiltered 8-bit RGB PNG, every chunk's CRC checked."""
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    at, chunks = 8, {}
    while at < len(data):
        size, = struct.unpack_from(">I", data, at)
        kind, body = data[at + 4:at + 8], data[at + 8:at + 8 + size]
        assert struct.unpack_from(">I", data, at + 8 + size)[0] == zlib.crc32(kind + body), kind
        chunks[kind] = chunks.get(kind, b"") + body
        at += 12 + size
    width, height, depth, colour = struct.unpack_from(">IIBB", chunks[b"IHDR"])
    assert (depth, colour) == (8, 2) and b"IEND" in chunks
    raw = zlib.decompress(chunks[b"IDAT"])
    rows = [raw[y * (1 + width * 3):(y + 1) * (1 + width * 3)] for y in range(height)]
    assert all(r[0] == 0 for r in rows)
    return width, height, b"".join(r[1:] for r in rows)


PIXELS = [[0xFF0000, 0x00FF00, 0x0000FF], [0x123456, 0xABCDEF, 0xFFFFFF]]  # 3 wide: each row padded by 3 bytes
RGB = bytes.fromhex("ff0000 00ff00 0000ff 123456 abcdef ffffff".replace(" ", ""))


class Readers(unittest.TestCase):
    def test_a_bmp_reads_from_the_top_row_down(self):
        self.assertEqual(media.read_bmp(bmp(3, 2, PIXELS)), (3, 2, RGB))
        self.assertEqual(media.read_bmp(bmp(3, 2, PIXELS, bits=32, top_down=True)), (3, 2, RGB))

    def test_a_bmp_it_cannot_read_is_refused(self):
        eight = bytearray(bmp(3, 2, PIXELS))
        eight[28] = 8
        for data in (b"PNG", bytes(eight), bmp(3, 2, PIXELS)[:-4]):
            with self.assertRaises(ValueError):
                media.read_bmp(data)

    def test_the_png_holds_the_pixels(self):
        self.assertEqual(png_pixels(media.png(3, 2, RGB)), (3, 2, RGB))

    def test_an_avi_gives_its_frames_and_rate_and_an_empty_chunk_repeats(self):
        first, second = b"\xff\xd8one\xff\xd9", b"\xff\xd8second\xff\xd9"  # 7 and 10 bytes: an odd one is padded
        got = media.read_avi(avi(64, 48, 5, [first, second, b"", second, first]))
        self.assertEqual((got.width, got.height, got.fps), (64, 48, Fraction(5)))
        self.assertEqual(got.frames, [first, second, second, second, first])

    def test_a_recording_cut_before_its_final_header_still_reads(self):
        frames = [b"\xff\xd8a\xff\xd9", b"\xff\xd8bb\xff\xd9"]
        self.assertEqual(media.read_avi(avi(64, 48, 10, frames, final=False)).frames, frames)

    def test_an_avi_of_another_codec_or_no_picture_is_refused(self):
        other = bytearray(avi(64, 48, 5, [b"x"]))
        other[112:116] = b"H264"
        for data in (bytes(other), avi(64, 48, 5, []), avi(64, 48, 5, [b""]), b"RIFF\0\0\0\0WAVE"):
            with self.assertRaises(ValueError):
                media.read_avi(data)


@unittest.skipUnless(shutil.which("ffmpeg") and shutil.which("ffprobe"), "the commands need ffmpeg and ffprobe")
class Commands(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="media-test-"))

    def tearDown(self):
        shutil.rmtree(self.dir)

    def run_media(self, *args):
        return subprocess.run([sys.executable, str(ROOT / "scripts/media.py"), *map(str, args)], check=True, capture_output=True, text=True).stdout

    def probe(self, path, entries="stream=codec_name,width,height,pix_fmt"):
        return subprocess.run(["ffprobe", "-v", "error", "-count_frames", "-show_entries", entries, "-of", "csv=p=0", str(path)],
                              check=True, capture_output=True, text=True).stdout.strip()

    def jpeg(self, colour):
        path = self.dir / f"{colour}.jpg"
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", f"color={colour}:size=321x181", "-frames:v", "1", str(path)], check=True)
        return path.read_bytes()

    def test_shots_writes_the_png_and_a_page_copy(self):
        (self.dir / "screen-001.bmp").write_bytes(bmp(3, 2, PIXELS))
        out = self.run_media("shots", self.dir / "screen-001.bmp", "--width", 2)
        self.assertEqual(png_pixels((self.dir / "screen-001.png").read_bytes()), (3, 2, RGB))
        self.assertEqual(self.probe(self.dir / "screen-001-2.png", "stream=width,height"), "2,2")
        self.assertIn("screen-001-2.png", out)

    def test_a_slideshow_loops_through_dark(self):
        for n, colour in enumerate((0x2040C0, 0xC04020)):
            (self.dir / f"{n}.bmp").write_bytes(bmp(64, 36, [[colour] * 64] * 36))
        self.run_media("slideshow", self.dir / "0.bmp", self.dir / "1.bmp", "-o", self.dir / "show", "--width", 64, "--hold", 1, "--fade", .5)
        data = (self.dir / "show.webp").read_bytes()
        frames, at = [], 12
        while at + 8 <= len(data):
            size, = struct.unpack_from("<I", data, at + 4)
            if data[at:at + 4] == b"ANMF":
                frames.append(int.from_bytes(data[at + 20:at + 23], "little"))  # the frame's duration in ms
            at += 8 + size + (size & 1)
        # Two pictures held, two transitions of 5 frames (the last back to the first): 2.5 s in all, looping.
        self.assertEqual(data[:4] + data[8:12], b"RIFFWEBP")
        self.assertEqual(sum(frames), 2500)
        self.assertGreater(len(frames), 4)

    def test_series_and_video_from_a_recording(self):
        red, blue = self.jpeg("red"), self.jpeg("blue")
        (self.dir / "record-001.avi").write_bytes(avi(321, 181, 10, [red, b"", blue, b"", red]))
        out = self.run_media("series", self.dir / "record-001.avi", "--width", 160)
        self.assertIn("5 frames at 10 a second, 3 pictures", out)
        self.assertEqual(len(list((self.dir / "record-001").glob("frame-*.png"))), 3)
        self.assertTrue(self.probe(self.dir / "record-001.gif").startswith("gif,160,90"))
        self.assertGreater((self.dir / "record-001.webp").stat().st_size, 0)
        self.run_media("video", self.dir / "record-001.avi", "--width", 200)
        self.assertEqual(self.probe(self.dir / "record-001.mp4"), "h264,200,112,yuv420p")
        frames = self.probe(self.dir / "record-001.mp4", "stream=nb_read_frames,r_frame_rate")
        self.assertEqual(frames, "10/1,5")


if __name__ == "__main__":
    unittest.main()
