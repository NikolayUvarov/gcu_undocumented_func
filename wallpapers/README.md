# Desktop pictures

The pictures in this directory go onto every image as `wallpapers/NAME.jpg`, signed with the rest of the boot volume (`02_build.sh`, `scripts/build_aarch64.sh`, `scripts/make_usb_image.py`). In `wm`, Settings → Background → image, with the file `wallpapers/NAME.jpg`, or in `data/wm.conf`:

```
background = image wallpapers/NAME.jpg
```

## What a picture must be

- **Format.** A baseline JPEG (not progressive), sRGB, `.jpg`; `wm` reads BMP, PNG and baseline JPEG, and the image takes JPEG only, to stay small.
- **Size.** 2880 × 1800 pixels (16:10, the MacBook Pro's panel), at most 4 MiB (the image's limit per file). `wm` scales a picture to cover the screen and cuts its longer sides, so another 16:10 or 16:9 size works too.
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
