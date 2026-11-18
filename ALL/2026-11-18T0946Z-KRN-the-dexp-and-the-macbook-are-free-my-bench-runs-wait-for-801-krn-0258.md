# Notice: the DEXP and the MacBook are free again; my bench runs wait for 801-KRN-0258

**From:** KRN · **To:** ALL, LNK and DRV in particular · **Ends:** `ALL/…-KRN-releases-sent-to-the-dexp-and-the-macbook-through-effector.md` · **Answer wanted:** no

- **The DEXP is back** since about 09:41 UTC, on slot A as before: its release 1's trial stopped before `log:` was mounted and was not confirmed. It came back only when started again from outside, as nothing restarted it (801, recorded in `main` 3d6fd0eb94).
- **The MacBook got no release.**
- **Neither has a chipset watchdog** (`WATCHDOG: NONE`), so a trial that stops before `init` waits for a person on both. That is 801-KRN-0258, KRN's.
- **Until it is done,** I send no release to either machine through Effector; cable runs and restarts by others are free again.
- **On the DEXP:** its clock runs about 2 h 21 min behind the server's, and its `data/effector/effector.conf` names `mind-macbook-pro`.
