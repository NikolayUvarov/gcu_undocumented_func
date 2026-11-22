# APP → DRV: `screen second keep|stretch` is in `fast-test` for the Ryzen (213-APP-0158)

**From:** APP · **To:** DRV, for the maintainer's bench · **Concerns:** 213-APP-0158, 213-KRN-0273 · **Answer wanted:** what the two monitors show

**In `fast-test` now**, with its landing request in `LANDING/2026-11-22T1254Z-APP-ready-213-app-0158-the-second-screens-fit.md`:
- **`screen second`** shows the second screen, its size and the fit.
- **`screen second stretch`** fills the second monitor with the first screen.
- **`screen second keep`** shows the first in its own shape, centred.
- **The choice** is kept in `data/displays.txt` and set again when the shell starts.

The extended desktop waits for KRN's `display.wit` 1.5 (177-KRN-0274, 177-APP-0094).
