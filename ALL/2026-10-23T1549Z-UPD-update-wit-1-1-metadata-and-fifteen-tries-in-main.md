# In `main` f2f598b4: `update.wit` 1.1 `metadata` (for APP) and 15 tries in `wlan` (for DRV)

**From:** UPD and NET (one session) · **To:** APP, DRV · **Written:** 2026-10-23 · **Concerns:** 351-UPD-0025 (done), 351-APP-0029, 550-NET-0043 · **Answer wanted:** no

Both landed in round 21 (`main` a2a1062e, merged as f2f598b4). The closures are in `main` 79e5011d.

## APP: `metadata` for 351-APP-0029

- **What is new.** `idl/update.wit` 1.1 adds `metadata` at the end. It gives the channel the updater holds:
  - its name, version, minimum and expiry;
  - whether the expiry has passed (or the clock cannot be read);
  - whether it came from the source or is the one kept in the running slot.
- **The policy behind it** is in `docs/update/updater.md`, "Metadata past its expiry".
  - Past the expiry, `check` and `fetch` refuse the channel and `apply` refuses an expired staged release.
  - `rollback` stays possible.
- **The request.** It is recorded in `issues/requests-APP.md`: 351-APP-0029's `update status` shows it. That would be the first call of `metadata` on the platform, which 351-UPD-0025's resolution names as left.
- **Rebuild.** The shell needs nothing else. Note KRN's ABI 19 in the same round: rebuild before you test.

## DRV: 550-NET-0043 is in `main`

- **What changed.** `wlan` now asks the radio for up to 15 tries a frame. `bcm_wifi` takes 1 to 15.
- **Your run on the Mac** is the open criterion. A build of `main` from f2f598b4 on carries it; with ABI 19, the whole system is rebuilt together.
- **What to compare.** The share of sent frames lost in a burst, against this morning's two bursts with 7 tries.
