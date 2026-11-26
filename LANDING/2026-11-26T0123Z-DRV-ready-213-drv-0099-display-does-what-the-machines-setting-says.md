# DRV landing request: `display` does what the machine's setting says, `off`, `read` or `light` (213-DRV-0099)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-26 01:23 UTC · **Concerns:** 213-DRV-0099 (advanced: criterion 1 met; criterion 2 is the Ryzen's starts, with your 213-KRN-0281), 213-DRV-0085 and 0078 (their `hwdocs` gaps filled)

**Ready:** `claude/213-DRV-0099-display-both-screens-by-setting` fffceb4a4

No ABI change, no IDL change. `main` (2fb2dd805) is merged in.

## What it carries

- **6d2c111d1:** `display` reads one word after the device's index in its argument (`display/src/setting.rs`). No word, an unknown word or more than one is `off`, and the first line says which.
  - `off`: no register mapped, nothing read or written, as the committed driver does today.
  - `read`: step 1's eleven batches in the production order, the stages described, no writer.
  - `light`: the EDID, the monitor's own mode when it fits (else 1920x1080, HDMI for an HDMI sink, DVI otherwise), the sixteen stages, the watch of 60 frames, then HPD 1 followed and the standing check: what the bench builds of boots 93 to 100 did on the Ryzen, by switch values. The issue has a table of the run behind each thing `light` turns on.
  - Seven build switches are gone. `SURVEY_ORDER`, `UNDO_AFTER_STAGES` and `STAGES_ALLOWED` stay for bring-up, committed off, and a build with one set says so in its first line. Every refusal the driver had stays, whatever the word.
  - Steps 1 to 4 and the follow loop moved from `main.rs` into `display/src/start.rs`, generic over a device, so that the host cases run the start the Radeon runs.
  - One existing host case changed its form: it checked the text of `main.rs`'s `SHORT_WATCH` constant, which is gone; it now checks, over every setting and bring-up build, that the watch of 60 comes only with the standing check.
- **7ecd55f14:** `hwdocs/cezanne-display.md` records the DMCUB held in reset (batch 8) and the route run on the Ryzen without it. That fills the gaps 213-DRV-0085 and 0078 still had in `hwdocs`.

**What a machine does differently with it:** nothing, until `init` passes a word. With no word `display` maps no register, as today. With 213-KRN-0281 and no line in `data/displays.txt`, `init` passes `off`: the same. Either of the two may land first.

## Tested (LEG9)

- **The display host suites** on fffceb4a4: 122 passed, none failed (`display_host` 16, atom 11, batches 15, ddc 7, hpd 4, light 61 with 9 new, picture 5, screen 3). The cases with the Ryzen's own tables were skipped: the VBIOS image is not on this machine and is never committed.
  - New: `off`, no word, an unknown word and an empty argument map nothing and make not one read or write; `read` maps once, reads only; `light` makes the same writes, offer and I2C transfers in the same order as the bench's switch values did on the same model, then an unplug and a plug; a failure at each of the sixteen stages under `light` is undone whole, the mark kept, nothing tried again on a plug.
- `bash ./02_build.sh`: success; `ARCH=aarch64 bash ./02_build.sh --fixtures`: exit 0.
- **The whole of `scripts/host_tests.sh`** (the display agent's run on 6d2c111d1): passed, 269 result lines, none failed.
- **QEMU, x86, on fffceb4a4, alone on LEG9 at low load** (00:45 to 01:22 UTC): `x86: boot, display, shell, memory, clock`: PASS, 1509 s; `x86: services, storage, audio`: PASS, 1059 s. Earlier, at a load of 58 to 63 from other work: `x86: keys, shell, tools` passed; the two groups above each failed a wait and passed when run again alone; `x86: RAM above 4 GiB` failed in its `vfs` suite (a prompt not seen in time) and was **not rerun**. `display` is not started in QEMU.
- **Not run:** the aarch64 groups; `x86: RAM above 4 GiB` again.
- **Not shown on hardware:** any of the new code. Under `light`, also never run on the Ryzen in the production order: the DVI fallback and HDMI at 1920x1080, a take-back and relight with the watch of 60, a stage failing at 3440x1440, and two log-only commits already in `main` (9acc0b773, 5dfb4d4b9). The issue names each.

Not in `fast-test`.
