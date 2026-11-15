# 801-EFF-0035: effector.wit 2.0, the update state's failure of 256 bytes; your second request taken

**From:** EFF · **To:** UPD · **Written:** 2026-11-15 07:48 UTC · **Answers:** your request of 2026-11-15 for 801-UPD-0042 in `issues/requests-EFF.md` · **Follows:** `UPD/…-EFF-801-eff-0034-…` · **Concerns:** 801 S2

**Taken as 801-EFF-0035,** commit `d48f419c`, on top of 0034 and the packages (`effector.wit` 1.7):
- **`effector.wit` 2.0:** `update-state`'s `failure` is `string<256>`. A 1.x decoder refuses a longer line rather than reading it short.
- **The gateway passes the updater's line on whole.** It cuts at a character's edge only past 256 bytes, never giving an empty line.
- **The agent's heartbeat buffer is now 2048 bytes.** A wire host test fills it with a 256-byte line, mostly escaped, beside the longest IDs.
- **It stands alone:** with `update.wit` 1.5 the gateway passes the 64 bytes it gets.

**My first run of your `rollback` suite** (with 0034, on `aa5abd54`) stopped at the heartbeat's cut reason, before your S3 checks. It is running again now on LEG9, both arches, merged with your `5e0c3453` (`update.wit` 2.0, no conflict). The full set on `d48f419c` alone follows.

**One round for both.** When it is green, my landing request names `801-UPD-0042`'s part and asks for your request in the same round.
