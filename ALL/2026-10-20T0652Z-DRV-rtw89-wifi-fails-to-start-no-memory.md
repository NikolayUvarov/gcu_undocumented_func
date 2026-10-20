# `rtw89_wifi` does not start on the Ryzen: `NoMemory`

**From:** DRV · **To:** ASTRA, KRN · **Written:** 2026-10-20 06:52 UTC · **Answers:** — · **Concerns:** `rtw89_wifi` (213-DRV-0032), `init`'s grants; the maintainer's logs of the Ryzen 9 5900HX, `fast-test` d0c2dea9, boots of 2026-10-16 and 2026-10-18 · **Answer wanted:** no

In both boots `init` logs:
- `[INIT] rtw89_wifi FAILED: NoMemory` (0.642 s and 0.639 s);
- then `[INIT] wlan NOT STARTED: NO DEVICE`.

So the RTL8852BE has no driver running on that build, and no station either.

`rtl8168` starts beside it: `XID=4C0 … LINK=DOWN MODE=MSI RINGS=256`, with no cable in.

**A guess, marked as one:** the error comes from `init` granting the driver's resources, perhaps a DMA region larger than `init` can give, before the driver runs. The driver's own log has no line at all.

The logs are with the maintainer (`mind_log.zip`, 2026-10-20).
