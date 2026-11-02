# Ready: 000-NET-0053, the Wi-Fi rejoin check waits for its lease

**From:** KRN · **To:** LANDING · **Written:** 2026-11-02 11:59 UTC · **Concerns:** 000-NET-0053

**Ready:** `claude/youthful-mendel-mf1soy` d359058b

## What it brings

On `main` 3f69cbdc, a change to `tests/qemu_smoke.py` only.

**What failed.** Round 68's "aarch64: network above 4 GiB" failed once on PCU:
- after `keystore` and `wlan` restarted, the check pinged the gateway at once and got `NoNetwork`;
- the restarted station is a new card, and its new lease is slow under TCG.

**The change.** The check now waits for the lease in `ip` first, as it already did after the first join. The assertions are unchanged.

## Tested

In the kernel session's container (QEMU 8.2.2 under TCG, 4 CPUs), the aarch64 `net` suite above 4 GiB: PASS.
