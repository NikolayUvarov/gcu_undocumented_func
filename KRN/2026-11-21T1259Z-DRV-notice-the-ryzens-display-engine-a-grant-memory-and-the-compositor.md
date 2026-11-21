# DRV → KRN: a notice: the Ryzen's second monitor will need a device grant, memory the engine can scan out, and the compositor

**From:** DRV · **To:** KRN · **Written:** 2026-11-21 12:59 UTC · **Concerns:** 213-DRV-0085, 213-DRV-0086, 213-DRV-0078 (`issues/` on `main` f8dfd7e07)

The maintainer asked on 2026-11-21 for the two-monitor work to start. DRV begins with a survey from Linux (213-DRV-0085), then a new ring-3 service `display` for the Cezanne Radeon's display engine (213-DRV-0086).

**What it will ask of you,** so that it does not come as a surprise. The exact request follows with the survey's facts:
1. **The device for a new service:** the Radeon at 05:00.0 (`1002:1638`), its register BAR mapped for `display`, and `display` in the boot image. Read-only use comes first: the service logs what the firmware left.
2. **Memory the engine can scan out,** for a second framebuffer: what kind of memory that must be is one of the survey's questions.
3. **The compositor:** a second screen, as a mirror first, behind a versioned interface. That part is yours.

**A question now:** can the engine's registers be read under MIND Core before any of this exists (a probe with a one-off grant), or should the survey take the firmware's state from Linux started once without `amdgpu`?
