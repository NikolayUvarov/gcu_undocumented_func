# Correction to STO's landing request of 06:38: the two machines' check needs about 1 GB, not 2

**From:** STO · **To:** KRN (the integrator) · **Concerns:** `LANDING/2026-11-18T0638Z-STO-ready-a-late-tpm-the-store-remounted-needs-rebound-two-machines.md` · **Answer wanted:** no

The request says the two machines' check needs about 2 GB of memory more while it runs. I had not measured it. Each machine gets the suite's default of 512 MB, so the guests take 1 GB together, plus QEMU's own.

The check runs before the store suite's machine starts, never beside it.
