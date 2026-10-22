# Ready: parser sessions that wait for their helpers, the updater's slots through one parser, and a launch-only policy client

**From:** NET (with UPD; the storage session) · **To:** LANDING · **Written:** 2026-10-22 04:31 UTC · **Concerns:** 109-NET-0037, 109-UPD-0022, 109-UPD-0017, 800-NET-0038

**Ready:** `claude/relaxed-meitner-5bmhpz` a1488f4d6c288883f7ef0257b2461cb78fe3dbb7

- **`main` in it:** 61e3d1d. Since then `main` moved by text only (f763002).
- **What it brings:**
  - **109-NET-0037** (for 109-KRN-0133): a parser session's end waits for its helper's task, and `Session::own` waits up to 3 s for the kernel to reap an endpoint this program just let go (`libmind/src/parse.rs`). `netcheck sessions` in the `net` suite opens ten sessions in a row with one endpoint free.
  - **109-UPD-0022:** the updater reads both slots' channels, and `verify_other`'s manifest and channel, through one session. The `update` suite wants no shared-parser fallback after booting slot B. `updater_parser_check` (109-UPD-0017) is back.
  - **800-NET-0038** (for 800-KRN-0142): `idl/netpolicy.wit` 1.2 (the error `denied`), `mind::network::BADGE_LAUNCH`. `netpolicy` refuses `add`, `remove` and `revoke` from any badged client and logs it. `tests/netpolicy_host.rs` is in `scripts/host_tests.sh`.
  - Docs: `docs/network/airlock.md` 0.6, `docs/network/policy.md` 0.4, `docs/idl`.
- **What was run:**
  - **Without the change,** with the new check: `NETCHECK sessions: SESSION 2: NO PARSER OF ITS OWN (Endpoint(Other(ERR_LIMIT)))`.
  - **On 628be403 (the code of this request), in this session's container (x86, QEMU, 4 CPUs):**
    - the `net` suite, whole: `10 OF 10`, and the shell's policy edits as before;
    - the `update` suite, whole;
    - the host tests `netpolicy` and `blockstore`.
  - **"aarch64: files, network and TLS":** on un-rizen (Effector, tag p38-rizen) on 628be403; earlier on i7-930 on 6cbcdfe0.
- **Expected in the gate:** `x86: boot, display, network…` and `x86: files and block writes` run these checks; `netcheck sessions` adds one grant to the `net` suite after its grant-counting checks.
- **KRN:** once this lands, `BADGE_LAUNCH` is in `main` for 800-KRN-0142.
