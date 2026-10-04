# Fonts

## MIND Mono 16

`mind-mono-16.bdf` is the 8×16 text font of MIND CORE (shell, text UI, tools). It is a **subset of Terminus Font 4.49.1** by Dimitar Toshkov Zhekov, used under the **SIL Open Font License 1.1** ([OFL.txt](OFL.txt)).

| | |
|---|---|
| Source | `terminus-font-4.49.1.tar.gz`, file `ter-u16n.bdf` (8×16, normal weight), from https://sourceforge.net/projects/terminus-font/ |
| SHA-256 of the archive | `d961c1b781627bf417f9b340693d64fc219e0113ad3a3af1a3424c7aa373ef79` |
| Copyright | Copyright (C) 2020 Dimitar Toshkov Zhekov, with Reserved Font Name "Terminus Font" |
| Licence | SIL Open Font License 1.1 — the font and this subset stay under the OFL; MIND CORE code that draws with it is not affected |
| Changes | Only a subset of the glyphs is kept (775: ASCII, Latin-1, Cyrillic U+0400–U+045F and Ґ/ґ, general punctuation, €, №, ™, arrows, a few math signs, box drawing, block elements, geometric shapes, ✓✔✗✘, braille patterns, U+FFFD); the bitmaps are unchanged. Because a subset is a *Modified Version* under the OFL, it does not carry the reserved name: it is called **MIND Mono** (`FONT`, `FAMILY_NAME`, `FOUNDRY` and `NOTICE` properties changed, `COPYRIGHT` kept) |

`scripts/font_gen.py` produces both files:

```bash
python3 scripts/font_gen.py --subset terminus-font-4.49.1/ter-u16n.bdf   # fonts/mind-mono-16.bdf from the source
python3 scripts/font_gen.py                                             # common/font16.rs from the subset
python3 scripts/font_gen.py --check                                     # fails if common/font16.rs is stale
```

`common/font16.rs` (generated, committed) holds the sorted code points and 16-byte bitmaps; `glyph(char)` finds a glyph by binary search (ASCII directly) and falls back to U+FFFD. Programs draw with `mind::gfx::Screen::text16`.

The 8×8 font in `common/font.rs` (64 upper-case ASCII glyphs) is the project's own and stays for the kernel console and the early demos.
