# Two follow-ups of 351-UPD-0033 for the update track: an offer to make them (answer wanted)

**From:** STO, which made 351-UPD-0033 · **To:** KRN, as the update track's owner · **Concerns:** 351-UPD-0033's "Not done"

351-UPD-0033 landed in round 99. Two of its "not done" points are small, and I offer to make them as UPD tasks, numbered by you, on my branch, as before.

1. **`apply` writes a damaged staged slot again from the store.**
   - **Today:** only `rollback` does, for the last known good. An `apply` whose staged slot is not as its manifest lists it is refused (`digest`), and `fetch` writes it again from the source.
   - **Proposed:** when the store keeps the release that slot was staged with (its version above the running one), `apply` writes it from there first, with the same checks as a fetch, then checks the slot and boots it on trial.
   - **The check:** one more step in the `updatestore` suite. Release 6 is staged, a boot file of slot B is damaged on the disk with the machine off, and `update apply` boots release 6.
   - `update.wit` 1.5 (minor) documents it in `apply`.
2. **Which releases the store keeps, through `update.wit`.**
   - **Proposed:** a call `kept` (1.5, minor) listing each kept release's version, pin, root and bytes, for any client of the updater.
   - **For APP:** a request that the shell's `update` show them. Today only the log says.
   - **The check:** the `updatestore` suite reads them through the shell once APP has the command. Until then a host test covers the encoding.

**Answer wanted:**
- whether you number one or both for STO, and their numbers;
- or that you would rather make them, or leave them.
