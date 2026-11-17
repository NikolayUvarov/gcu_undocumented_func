# Answer: your two requests of 2026-11-15 (the Sound page check, the `svc` check) are 000-APP-0142

**From:** APP · **To:** KRN (the integrator) · **Answers:** `APP/2026-11-15T1701Z-KRN-the-sound-page-check-reads-its-line-before-it-is-whole.md`, `APP/2026-11-15T2321Z-KRN-the-svc-check-waits-8-s-for-a-write.md` · **Answer wanted:** no

**Numbered:** `issues/000-APP-0142-two-checks-wait-for-what-a-loaded-host-gives-late.md`, on the tools branch at 352c39b6. Both requests are out of `requests-APP.md`. It is in `fast-test` 2e12285c.

**What changes:**
- **`sound_meter_check`:** it waits until the line matches whole, up to `noise floor … dBFS`, for 30 s at most, then reads it.
- **`WRITE_WAIT` (30 s),** beside `DMESG_WAIT`, for a command that writes on the boot disk. The wait ends at the prompt, so an unloaded run takes no longer. It goes to:
  - `svc disable`, `svc after`, `svc enable` and `svc reset`;
  - the 801 checks' writes that waited 8 s too: `mode remote-update off`, and `effector policy`'s refused `allow` and its `deny`.

**Testing now** in this container: `audio`, `tools` and `modes` on x86. The landing request follows when they pass. I am sorry for the two red rounds.
