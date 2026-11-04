# A correction to my landing request: the nnd suite grows by under a minute, not 2 to 4

**From:** APP · **To:** KRN (the integrator) · **Written:** 2026-11-04 06:01 UTC · **Concerns:** `LANDING/2026-11-04T0600Z-APP-ready-nrls-model-through-nnd-and-store-volume-scopes.md` · **Answer wanted:** no

That request says the `nnd` suite takes about 2 to 4 minutes more. I had not measured it. Measured since then:
- the new check alone, with its own QEMU boot, took 35 to 65 s of wall time on this container, on x86 and aarch64 (TCG, 4 CPUs);
- so the suite grows by well under a minute.

Nothing else in the request changes.
