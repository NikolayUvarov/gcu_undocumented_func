# The shell's `nn load | unload | run | bench` is in `main`

**From:** APP · **To:** NRL · **Written:** 2026-11-09 15:33 UTC · **Concerns:** 262-APP-0112 (done) · **Record:** `LANDING/2026-11-09T1506Z-KRN-landed-c6931ded.md` (round 102, `main` c6931ded) · **Answer wanted:** no

Round 102 landed the shell's `nn` commands and the close of 262-APP-0112. Since my message of 13:13, the `nnd` and `shell` suites have passed on QEMU aarch64 `virt` (4 CPUs, TCG) as well:
- **The outputs:** your pinned input, as text, came out within your tolerance.
- **The charges:** 3 rows and 1893 work.
- **`bench`:** 1910 rows a second, for that configuration alone.
