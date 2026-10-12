"""Writes the pictures of tests/image_host.rs and the wm suite (000-APP-0047, 000-APP-0050) with Pillow, and libjpeg's
own decoding of each JPEG beside it as RGB bytes (`<name>.rgb`). Run from anywhere: python3 tests/data/images/make.py"""
from pathlib import Path
import zlib
from PIL import Image

HERE = Path(__file__).resolve().parent
W, H = 40, 24


def small(x, y):
    return ((x * 6) & 255, (y * 10) & 255, ((x + y) * 4) & 255)


def big(x, y):
    n = (x * 7919 + y * 104_729 + x * y * 31) % 251
    return ((x * 3 + n // 8) & 255, (y * 5 + n // 4) & 255, n & 255)


def picture(w, h, pixel, mode="RGB"):
    im = Image.new("RGB", (w, h))
    im.putdata([pixel(x, y) for y in range(h) for x in range(w)])
    return im if mode == "RGB" else im.convert(mode)


def save_jpeg(im, name, **options):
    im.save(HERE / name, quality=75, **options)
    (HERE / f"{name}.rgb").write_bytes(Image.open(HERE / name).convert("RGB").tobytes())


rgb, large = picture(W, H, small), picture(160, 96, big)
rgb.save(HERE / "rgb.png")
rgb.convert("RGBA").save(HERE / "rgba.png")
rgb.convert("L").save(HERE / "grey.png")
Image.frombytes("I;16", (W, H), b"".join((x * 1500 + y * 100).to_bytes(2, "little") for y in range(H) for x in range(W))).save(HERE / "grey16.png")
rgb.quantize(16).save(HERE / "palette.png", bits=4)
large.save(HERE / "big.png")
large.save(HERE / "stored.png", compress_level=0)
save_jpeg(rgb, "plain.jpg")
save_jpeg(rgb, "444.jpg", subsampling=0)
save_jpeg(rgb.convert("L"), "grey.jpg")
save_jpeg(rgb, "restart.jpg", restart_marker_blocks=3)
save_jpeg(large, "big.jpg")
save_jpeg(large, "big444.jpg", subsampling=0)
rgb.save(HERE / "progressive.jpg", quality=75, progressive=True)
# Two flat halves on block edges: the wm suite's PNG and JPEG backgrounds, decoded exactly.
halves = picture(64, 40, lambda x, y: (0xC0, 0x80, 0x40) if x < 32 else (0x40, 0x80, 0xC0))
halves.save(HERE / "halves.png")
halves.save(HERE / "halves.jpg", quality=95, subsampling=0)
# Fixed Huffman codes only.
packer = zlib.compressobj(9, zlib.DEFLATED, 15, 9, zlib.Z_FIXED)
(HERE / "fixed.zlib").write_bytes(packer.compress(b"MIND Core " * 50 + bytes(range(256))) + packer.flush())
