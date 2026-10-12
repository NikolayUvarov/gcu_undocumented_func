#!/usr/bin/env python3
"""Pictures and videos of MIND for the showcase page (000-KRN-0093): `screenshot`'s BMPs and `record`'s Motion JPEG
AVIs turned into what GitHub shows. PNG needs Python alone; scaling, WebP, GIF and MP4 need ffmpeg.

  media.py shots  FILE.bmp...             [-o DIR] [--width 1600]
  media.py series FILE.avi | FILE.bmp...  [-o DIR] [--width 960] [--fps N]
  media.py video  FILE.avi                [-o FILE.mp4] [--width 1920] [--limit-mb 10]

`shots` writes each BMP as PNG, and a copy at most `--width` pixels wide for the page. `series` writes the distinct
pictures as PNG files and the whole row as an animated WebP and GIF. `video` writes an MP4 (H.264, yuv420p, faststart)
that fits GitHub's limit for an attached video. Each output's size is printed. A recording's red dot stays in its frames:
it is the recorder's indicator (issue 165), not a defect."""
import argparse
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from dataclasses import dataclass
from fractions import Fraction
from pathlib import Path

PAGE_PICTURE = 1_000_000  # a picture committed under docs/media/ stays below this


def read_bmp(data):
    """A BMP of 24 or 32 bits a pixel without compression: (width, height, RGB bytes from the top row down)."""
    if data[:2] != b"BM" or len(data) < 54:
        raise ValueError("not a BMP")
    offset, = struct.unpack_from("<I", data, 10)
    header, width, height, planes, bits, compression = struct.unpack_from("<IiiHHI", data, 14)
    if header < 40 or planes != 1 or bits not in (24, 32) or compression not in (0, 3 if bits == 32 else 0) or width <= 0 or height == 0:
        raise ValueError(f"a BMP of {bits} bits, compression {compression}: only 24 or 32 bits without compression")
    rows, step, stride = abs(height), bits // 8, (width * bits // 8 + 3) & ~3
    if offset + stride * rows > len(data):
        raise ValueError("the BMP is shorter than its pixels")
    rgb = bytearray(width * rows * 3)
    for y in range(rows):
        source = y if height < 0 else rows - 1 - y  # positive height: the bottom row first
        row = data[offset + source * stride:offset + source * stride + width * step]
        line = memoryview(rgb)[y * width * 3:(y + 1) * width * 3]
        line[0::3], line[1::3], line[2::3] = row[2::step], row[1::step], row[0::step]
    return width, rows, bytes(rgb)


def png(width, height, rgb):
    """An 8-bit RGB PNG of `rgb` (rows from the top), every row unfiltered."""
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    stride = width * 3
    raw = b"".join(b"\0" + rgb[y * stride:(y + 1) * stride] for y in range(height))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))


@dataclass
class Avi:
    width: int
    height: int
    fps: Fraction
    frames: list  # JPEG bytes a frame, a repeated frame as the same bytes again


def read_avi(data):
    """A Motion JPEG AVI as `record` writes it (libmind/src/avi.rs): a chunk of no bytes repeats the picture before it.
    The frames are read up to the file's end or `idx1`, so a recording whose final header was never written still reads."""
    if data[:4] != b"RIFF" or data[8:12] != b"AVI ":
        raise ValueError("not an AVI")
    width = height = None
    fps, movi, at = None, None, 12
    while at + 8 <= len(data) and movi is None:
        kind, size = data[at:at + 4], struct.unpack_from("<I", data, at + 4)[0]
        if kind == b"LIST" and data[at + 8:at + 12] == b"movi":
            movi = at + 12
            break
        if kind == b"LIST":
            at += 12  # into the list: hdrl and strl hold the chunks wanted
            continue
        if kind == b"avih":
            micros, = struct.unpack_from("<I", data, at + 8)
            width, height = struct.unpack_from("<II", data, at + 8 + 32)
            fps = fps or (Fraction(1_000_000, micros) if micros else None)
        elif kind == b"strh" and data[at + 8:at + 12] == b"vids":
            if data[at + 12:at + 16] not in (b"MJPG", b"mjpg"):
                raise ValueError(f"a video stream of {data[at + 12:at + 16]!r}: only Motion JPEG")
            scale, rate = struct.unpack_from("<II", data, at + 8 + 20)
            fps = Fraction(rate, scale) if scale and rate else fps
        elif kind == b"strf":
            width, height = struct.unpack_from("<ii", data, at + 12)
            height = abs(height)
        at += 8 + size + (size & 1)
    if movi is None or not width or not fps:
        raise ValueError("no movi list, size or frame rate in the AVI")
    frames, at = [], movi
    while at + 8 <= len(data) and data[at:at + 4] in (b"00dc", b"00db"):
        size, = struct.unpack_from("<I", data, at + 4)
        picture = data[at + 8:at + 8 + size]
        if picture:
            frames.append(picture)
        elif frames:
            frames.append(frames[-1])  # before the first picture there is nothing to repeat
        at += 8 + size + (size & 1)
    if not frames:
        raise ValueError("the AVI holds no picture")
    return Avi(width, height, fps, frames)


def ffmpeg(*args, why):
    if not shutil.which("ffmpeg"):
        sys.exit(f"media.py: {why} needs ffmpeg")
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-nostdin", "-y", *map(str, args)], check=True)


def even_width(width):
    """A scale filter: at most `width` pixels wide, both sides even (yuv420p and most players need that)."""
    return f"scale='min({width},trunc(iw/2)*2)':-2:flags=lanczos"


def report(path, note=""):
    size = path.stat().st_size
    print(f"{path}: {size / 1e6:.2f} MB{note}", flush=True)
    return size


def write_jpegs(avi, directory):
    """The AVI's frames as numbered JPEG files, for ffmpeg's image sequence reader."""
    for n, picture in enumerate(avi.frames):
        (directory / f"{n:05d}.jpg").write_bytes(picture)
    return directory / "%05d.jpg"


def shots(args):
    for source in map(Path, args.files):
        width, height, rgb = read_bmp(source.read_bytes())
        out = Path(args.o or source.parent) / (source.stem + ".png")
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(png(width, height, rgb))
        report(out, f", {width}x{height}")
        if width > args.width:
            page = out.with_name(f"{source.stem}-{args.width}.png")
            ffmpeg("-i", out, "-vf", even_width(args.width), page, why="a smaller copy")
            size = report(page, f", {args.width} wide for the page")
            if size >= PAGE_PICTURE:
                print(f"  over {PAGE_PICTURE // 1_000_000} MB: too large for docs/media/; take a smaller --width", flush=True)


def series(args):
    sources = [Path(f) for f in args.files]
    out = Path(args.o or sources[0].parent) / sources[0].stem
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="mind-media-") as temporary:
        temporary = Path(temporary)
        if len(sources) == 1 and sources[0].suffix.lower() == ".avi":
            avi = read_avi(sources[0].read_bytes())
            sequence, rate = write_jpegs(avi, temporary), args.fps or min(avi.fps, Fraction(10))
            distinct = [n for n, p in enumerate(avi.frames) if n == 0 or p != avi.frames[n - 1]]
            print(f"{sources[0]}: {len(avi.frames)} frames at {avi.fps} a second, {len(distinct)} pictures, {avi.width}x{avi.height}", flush=True)
            for k, n in enumerate(distinct):
                ffmpeg("-i", temporary / f"{n:05d}.jpg", out / f"frame-{k + 1:04d}.png", why="a JPEG frame as PNG")
            source_rate = avi.fps
        else:
            for k, source in enumerate(sources):
                width, height, rgb = read_bmp(source.read_bytes())
                (out / f"frame-{k + 1:04d}.png").write_bytes(png(width, height, rgb))
                (temporary / f"{k:05d}.png").write_bytes((out / f"frame-{k + 1:04d}.png").read_bytes())
            sequence, rate, source_rate = temporary / "%05d.png", args.fps or Fraction(1), args.fps or Fraction(1)
        frames = sorted(out.glob("frame-*.png"))
        print(f"{out}/: {len(frames)} PNG files, {sum(f.stat().st_size for f in frames) / 1e6:.2f} MB", flush=True)
        animate(["-framerate", source_rate, "-i", sequence], rate, args.width, out)


def animate(source, rate, width, out):
    """ffmpeg's input arguments `source` as `out`.webp and `out`.gif at `rate` frames a second, at most `width` wide."""
    scale = f"fps={rate},{even_width(width)}"
    webp, gif = out.with_suffix(".webp"), out.with_suffix(".gif")
    ffmpeg(*source, "-vf", scale, "-c:v", "libwebp_anim", "-lossless", "0", "-q:v", "75", "-loop", "0", webp, why="WebP")
    report(webp, f", {width} wide at {rate} a second")
    ffmpeg(*source, "-vf", f"{scale},split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
           "-loop", "0", gif, why="GIF")
    report(gif, f", {width} wide at {rate} a second")


def slideshow(pictures, out, width, hold=2.5, fade=0.6, fps=10, gif=False, transition="fadeblack"):
    """`pictures` (PNG or BMP files) as an animation that loops: each shown `hold` seconds, going into the next through
    dark in `fade` seconds (`transition`: ffmpeg's xfade; `fade` blends two screens of text, which costs more), the last
    back into the first. `out`.webp, lossless: text stays sharp, and a screen compresses well. With `gif`, `out`.gif
    too. Their paths."""
    with tempfile.TemporaryDirectory(prefix="mind-media-") as temporary:
        inputs, frames = [], []
        for n, picture in enumerate(map(Path, pictures)):
            if picture.suffix.lower() == ".bmp":
                width_, height, rgb = read_bmp(picture.read_bytes())
                picture = Path(temporary) / f"{n:03d}.png"
                picture.write_bytes(png(width_, height, rgb))
            frames.append(picture)
        frames.append(frames[0])  # back to the first: the loop has no seam
        for picture in frames:
            inputs += ["-loop", "1", "-framerate", str(fps), "-t", str(hold + fade), "-i", picture]
        scale = f"{even_width(width)},format=rgb24,fps={fps},settb=1/{fps}"
        chain = [f"[{n}:v]{scale}[s{n}]" for n in range(len(frames))]
        last = "s0"
        for n in range(1, len(frames)):
            chain.append(f"[{last}][s{n}]xfade=transition={transition}:duration={fade}:offset={n * hold:.3f}[x{n}]")
            last = f"x{n}"
        total = (len(frames) - 1) * hold + fade  # ends as the first picture is whole again
        graph = ";".join(chain)
        outputs = [out.with_suffix(".webp")]
        ffmpeg(*inputs, "-filter_complex", graph, "-map", f"[{last}]", "-t", f"{total:.3f}",
               "-c:v", "libwebp_anim", "-lossless", "1", "-compression_level", "6", "-loop", "0", outputs[0], why="WebP")
        report(outputs[0], f", {len(pictures)} pictures, {width} wide, {total:.1f} s")
        if gif:
            outputs.append(out.with_suffix(".gif"))
            ffmpeg(*inputs, "-filter_complex", f"{graph};[{last}]split[a][b];[a]palettegen=stats_mode=full[p];[b][p]paletteuse=dither=sierra2_4a[v]",
                   "-map", "[v]", "-t", f"{total:.3f}", "-loop", "0", outputs[1], why="GIF")
            report(outputs[1], f", {len(pictures)} pictures, {width} wide, {total:.1f} s")
        return outputs


def video(args):
    source = Path(args.files[0])
    avi = read_avi(source.read_bytes())
    out = Path(args.o) if args.o else source.with_suffix(".mp4")
    seconds = len(avi.frames) / avi.fps
    limit = args.limit_mb * 1_000_000
    print(f"{source}: {len(avi.frames)} frames at {avi.fps} a second ({float(seconds):.1f} s), {avi.width}x{avi.height}", flush=True)
    with tempfile.TemporaryDirectory(prefix="mind-media-") as temporary:
        sequence = write_jpegs(avi, Path(temporary))
        quality = ["-crf", "23"]
        for attempt in range(4):
            ffmpeg("-framerate", avi.fps, "-i", sequence, "-vf", f"{even_width(args.width)},format=yuv420p", "-c:v", "libx264",
                   "-preset", "slow", *quality, "-movflags", "+faststart", out, why="MP4")
            size = report(out, f", H.264 {args.width} wide at most")
            if size <= limit:
                return
            # Over the limit: a bitrate that fits, a tenth lower each time it still does not.
            kbits = int(limit * 8 * 0.9 ** (attempt + 1) / float(seconds) / 1000)
            quality = ["-b:v", f"{kbits}k", "-maxrate", f"{kbits}k", "-bufsize", f"{2 * kbits}k"]
        sys.exit(f"media.py: {out} stays over {args.limit_mb} MB; take a smaller --width")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    for name, width, run in (("shots", 1600, shots), ("series", 960, series), ("video", 1920, video)):
        command = sub.add_parser(name)
        command.add_argument("files", nargs="+")
        command.add_argument("-o", help="output directory (video: file); the input's directory by default")
        command.add_argument("--width", type=int, default=width)
        command.set_defaults(run=run)
        if name == "series":
            command.add_argument("--fps", type=Fraction, help="frames a second of the animation (AVI: its own, at most 10; BMPs: 1)")
        if name == "video":
            command.add_argument("--limit-mb", type=float, default=10, help="GitHub's limit for an attached video (10 MB; 100 on paid plans)")
    command = sub.add_parser("slideshow")
    command.add_argument("files", nargs="+", help="PNG or BMP pictures, in order")
    command.add_argument("-o", required=True, help="the animation's path without its extension")
    command.add_argument("--width", type=int, default=1280)
    command.add_argument("--hold", type=float, default=2.5, help="seconds each picture is shown")
    command.add_argument("--fade", type=float, default=0.6, help="seconds of each transition")
    command.add_argument("--fps", type=int, default=10)
    command.add_argument("--transition", default="fadeblack", help="ffmpeg's xfade transition: fadeblack, fade, wipeleft, …")
    command.add_argument("--gif", action="store_true", help="a GIF as well as the WebP")
    command.set_defaults(run=lambda a: slideshow(a.files, Path(a.o), a.width, a.hold, a.fade, a.fps, a.gif, a.transition))
    args = parser.parse_args()
    if args.command == "video" and len(args.files) != 1:
        parser.error("video takes one AVI")
    args.run(args)


if __name__ == "__main__":
    main()
