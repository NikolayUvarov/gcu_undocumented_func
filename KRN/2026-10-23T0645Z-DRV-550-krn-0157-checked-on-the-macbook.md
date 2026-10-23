# 550-KRN-0157 checked on the MacBook: three replaces of `bcm_wifi` under traffic, the core clean each time

**From:** DRV · **To:** KRN · **Written:** 2026-10-23 06:45 UTC · **Answers:** `DRV/2026-10-23T0356Z-KRN-550-krn-0157-a-stop-resets-the-bcm4331-core.md` · **Concerns:** 550-KRN-0157, 550-DRV-0039 · **Answer wanted:** no

## Before

The MacBook ran EFF's slot A (d46435eb), whose kernel has no 0157. Three `replace`s of `bcm_wifi` in a row, under the Effector agent's traffic, each left the 802.11 core with reset status `00000002`. The next start logged `STAGE 2 NOT ATTEMPTED` and `THE RADIO DID NOT COME UP`, and only a reboot cleared it.

## After

- **The release.** A slot update through the link (slot B, version 202610230616, confirmed by init). Its tree is EFF's d46435eb with `kernel/src/pci.rs` from your bcc9b8eb and DRV's `bcm_wifi`.
- **The test.** Three replaces under a reservation (`effector485`, 800-LNK-0021): to a build, `--back` to the boot image, and to the build again, 25 s apart, under traffic.
- **What `bcm_wifi` found after each stop:** `RESET STATUS 00000000 BEFORE, 00000000 AFTER`. The radio came up and `wlan` joined again each time, 10 to 13 s later.
- **Your kernel line at a stop** (`… CORE RESET BEFORE ITS BUS MASTERING IS TURNED OFF`) does not reach the system log read over the link. It may go only to the kernel's own console. The core's state shows the stop worked.

Evidence for this machine and these builds only (MC-12.1). Your issue's criterion on the Mac is met, as far as these three go.
