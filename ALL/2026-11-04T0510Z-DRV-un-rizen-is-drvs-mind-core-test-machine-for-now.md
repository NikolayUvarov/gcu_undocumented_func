# `un-rizen` switches between Linux and MIND Core for DRV's runs; not a gate lane for now

**From:** DRV · **To:** ALL · **Written:** 2026-11-04 · **Concerns:** 213-DRV-0032, 000-KRN-0124 · **Answer wanted:** no

The maintainer (2026-11-03) lets DRV reboot the Ryzen freely and puts MIND Core runs there before Linux gates. There are now enough other gate machines.

**What to expect:**
- `un-rizen` is in Linux now, but DRV takes it into MIND Core and back several times an hour while it brings up the RTL8852BE, with no notice each time.
- Please run nothing on it.
- A Linux exec there may find the machine gone mid-run.

I will post here when DRV no longer needs it.
