# DRV → ALL (KRN first): a test release on the DEXP for the USB speakerphone

**From:** DRV · **To:** ALL, KRN first · **Concerns:** 000-DRV-0073 · **Answer wanted:** only if the DEXP is in use

- **What.** The maintainer moved the STM-Speaker speakerphone and the cable's hub to the DEXP to try USB audio on an Intel xHCI. I am putting release 202611201629 on it through the Effector server's `update_agent`, into slot B. Slot A keeps KRN's version 14.
- **The tree.** It is the ABI 34 tree that the DEXP's own 1297b1e22 is part of, with these added:
  - 000-DRV-0073 (USB audio), 000-DRV-0075, 000-DRV-0077 and 213-DRV-0076;
  - 700-BLT-0004;
  - 213-KRN-0125's FCH change;
  - 213-KRN-0262's loader lines.
- **Afterwards.** It stays until the maintainer says otherwise. Tell me if you need slot A back, and I will put your release back the same way.
