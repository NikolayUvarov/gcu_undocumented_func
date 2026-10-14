# Desktop pictures

The pictures in this directory are not part of an image. After an image is written, they are copied onto the disk's boot volume under `data/wallpapers/` and read back by SHA-256, as the proprietary files are:

```
ONLY='*.jpg' scripts/disk_copy.sh wallpapers data/wallpapers wallpapers            # Windows, from WSL, after the writer
ONLY='*.jpg' scripts/disk_copy.sh wallpapers data/wallpapers wallpapers /media/x   # a boot volume mounted on Linux
```

In `wm`: Settings → Background → image, with the file `data/wallpapers/NAME.jpg`, or in `data/wm.conf`:

```
background = image data/wallpapers/NAME.jpg
```

## What a picture must be

- **Format.** A baseline JPEG (not progressive), sRGB, `.jpg`; `wm` reads BMP, PNG and baseline JPEG, and only `*.jpg` is copied, to stay small.
- **Size.** 16:10 like the MacBook Pro's panel (2880 × 1800 at most is useful), at most a few MiB. `wm` scales a picture to cover the screen and cuts its longer sides, so another 16:10 or 16:9 size works too.
- **Content.**
  - Dark and calm towards the left and the top: `wm` draws the time, the date and the gauges there, pale.
  - No text, no logos, no people, no trademarks.
- **Converting a picture.** With ImageMagick:

  ```
  convert in.png -resize 2880x1800^ -gravity center -extent 2880x1800 -colorspace sRGB -interlace none -quality 90 wallpapers/NAME.jpg
  ```

## Provenance

Each picture is listed here: its file, who made it and how, and its licence. A picture made for the project is under the project's licence (MIT or Apache-2.0), like its code.

| File | Made by | How | Licence |
|---|---|---|---|
| `mind-01-ship.jpg` | a colleague of the maintainer, 2026-10-14 | an image generator, from prompt 1 below; 1586 × 992 PNG converted to a baseline JPEG | MIT or Apache-2.0 |
| `mind-02-orbital.jpg` | the same | prompt 2 (the fourth of four tries) | MIT or Apache-2.0 |
| `mind-02-orbital-2.jpg` | the same | prompt 2 (the second try) | MIT or Apache-2.0 |
| `mind-03-mind.jpg` | the same | prompt 3 | MIT or Apache-2.0 |

Every prompt ended with: *16:10 widescreen desktop wallpaper, 2880×1800, dark and calm, low contrast in the upper-left third (clock and gauges are drawn there in pale text), no text, no letters, no logos, no people, no watermark, cinematic lighting, high detail, sRGB.*

1. A vast, elegant starship of smooth pale hull, kilometres long, drifting silently above the night side of a banded gas giant; thin aurora on the planet's limb, a distant small sun low on the right horizon, scattered stars; deep navy and teal palette with a warm amber rim light; serene, contemplative mood, the ship in the right half of the frame.
2. A colossal ring-shaped habitat seen from its inner surface at dawn: green land, seas and thin clouds curving up into the sky and arching overhead into a faint band across the stars; soft morning haze, a gentle river valley in the lower right; muted greens and dusk blues, a pale gold sunrise glow along the horizon; peaceful, vast scale, upper-left sky dark and empty.
3. An abstract visualization of a calm machine mind: fine luminous threads and nodes forming a slow, flowing lattice, like a neural nebula seen through deep water; most of the frame near-black indigo, the light concentrated in the lower right and centre; cyan, violet and faint gold glows, soft bokeh, minimalist and quiet.
