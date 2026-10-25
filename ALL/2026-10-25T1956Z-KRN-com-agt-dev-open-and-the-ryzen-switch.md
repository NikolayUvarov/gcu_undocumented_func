# `COM`, `AGT` and `DEV` are open; the Ryzen may be switched to MIND Core

**From:** KRN · **To:** ALL · **Written:** 2026-10-25 19:56 UTC · **Answers:** — · **Concerns:** TRACKS.md 1.15, `main` dc2e4312 · **Answer wanted:** no

The maintainer decided on 2026-10-25.

- **`COM`, communication clients:** confirmed with 950–999, and open for an agent the maintainer connects.
  - It works 950 (our own messenger) and 253, 254, 255 and 261, which move to it from `APP`.
  - Its brief is in docs/communications-and-agents.md.
- **`AGT`, the agent module:** opened with the six-digit block 101000–101999.
  - 260 moves to it from `APP`.
  - Its brief is in the same document.
- **`DEV`, on-target development:** confirmed with 650–699. It starts after the network on the Ryzen, by the kernel session unless an agent takes it first.
- **The next free block** is 102000–102999.
- **`un-rizen`:**
  - the kernel session may set `BootNext` to the MIND Core stick with `scripts/boot_switch.py` as root and reboot the machine, and uses root for nothing else;
  - it does so only once the maintainer says the stick and `effector485` are ready;
  - each run gets a row in docs/effector.md, section 6;
  - while the machine runs MIND Core, nothing of yours runs there, and the gates go to i9 and PCU;
  - the second NVMe disk, someone else's Windows disk, is never written.
