# Pictures and videos of MIND

How the pictures and videos on the project's page are made (000-KRN-0093). Russian: [media_RU.md](media_RU.md).

Each picture says where it was taken: QEMU's machine or a named real one (MC-12.1). A picture shows what one build did on one machine, not what every machine does.

## 1. In QEMU: the same scenes for each update

`scripts/qemu_scenes.py` boots the current x86 build (`usb_root`) in the test suites' machine with a 1920 × 1080 screen and saves fixed scenes as PNG:

| Scene | What it shows |
| --- | --- |
| `boot` | the shell after the boot, with the services' lines |
| `check` | `check kernel`'s table |
| `top` | `top` with `clock` running |
| `wm` | `wm` with `fm`, `clock`, `top` and the shell's window |
| `series` | `wm`'s windows changing, a frame every half second, also as an animated WebP and GIF |
| `tour` | the page's animation: the boot, `check`, `hw`, `top`, `wm` and three of its windows full screen, each going into the next through dark; lossless WebP, 1280 wide (the shell's screens cut at their top left, `wm`'s scaled). It plays its own scenes, so it runs alone |

```bash
./02_build.sh
python3 scripts/qemu_scenes.py -o media-scenes            # every scene, about 30 s
python3 scripts/qemu_scenes.py --scenes wm,series          # some of them
python3 scripts/qemu_scenes.py --scenes tour -o media-scenes && cp media-scenes/tour.webp docs/media/   # the page's animation
```

It takes the same `--qemu`, `--firmware` and `--cpus` as the suites, and `--size` for another screen. ffmpeg makes the WebP and GIF.

## 2. On a real machine

Files written to `data/` stay on the boot disk, which Windows and macOS can read. Files on `ram:` are lost at a reboot.

- **A picture of the shell's screen:** `screenshot data/screen-001.bmp`. It captures the screen in front, which is the shell's when typed there: the boot lines, `check`'s or `bench`'s tables.
- **A recording of the screen:** `run record -r 5 -t 20 data/wm.avi &`, then the program to show, for example `wm`. `record` captures the screen in front for the whole time, whatever is in front, and the compositor shows a red dot in the corner meanwhile. `-r` is frames a second, `-t` seconds.
- **A recording of one window:** `record -w -t 10 data/window.avi` in `wm`'s run line (Alt+R). It records the window in front alone, at its content's size.
- **A picture of `wm`:** record a few seconds of it, then take its frames with `media.py series` (below) and choose one.

A screenshot by a key, with no shell on the screen, is asked of the tools track ([requests-APP.md](../issues/requests-APP.md)).

On the MacBook Pro the screen is 2880 × 1800: a BMP is about 15 MB. A recording grows with what changes on the screen: a frame like the one before it takes no bytes.

## 3. Converting: `scripts/media.py`

```bash
python3 scripts/media.py shots  screen-001.bmp              # screen-001.png, and screen-001-1600.png for the page
python3 scripts/media.py series wm.avi                      # wm/frame-NNNN.png (each distinct picture), wm.webp, wm.gif
python3 scripts/media.py series a.bmp b.bmp c.bmp --fps 1   # a row of screenshots, a second each
python3 scripts/media.py video  wm.avi                      # wm.mp4: H.264, yuv420p, at most 1920 wide, under 10 MB
python3 scripts/media.py slideshow a.png b.png -o show      # show.webp: each 2.5 s, into the next through dark, looping
```

- **Python alone** reads the BMPs and AVIs and writes the full-size PNGs. ffmpeg scales, and writes WebP, GIF and MP4.
- **Each output's size** is printed. `--width` sets the largest width; `video --limit-mb 100` is GitHub's limit on paid plans.
- **The red dot** stays in recorded frames: it is the recorder's indicator, not a defect.

`tests/media_test.py` checks the readers and the PNG writer on small files it makes, and the three commands where ffmpeg is installed.

## 4. Where the results go

- **Pictures** for the page go to `docs/media/` in this repository, each under 1 MB (the page copy from `shots`, or a frame from `series`), and the page's animation under 2 MB (`docs/media/tour.webp`). The showcase copies them with `main`.
- **Videos** are attached on GitHub (an issue, a release or the page's editor), not committed: an MP4 under 10 MB.
- **Under each picture:** the machine (QEMU, or the real machine's model), the screen size and the build's commit.
