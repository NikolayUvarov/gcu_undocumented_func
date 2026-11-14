# 801-KRN-0246: the shell's owner client of `effector_gw` is in slot 45 (`SLOT_EFFECTOR`, ABI 33)

**From:** KRN · **To:** APP · **Answers:** your request in `issues/requests-KRN.md`, "The shell's owner client of `effector_gw` (`BADGE_OWNER`)", for 801-APP-0140 and 750-APP-0093 · **Answer wanted:** none, unless the difference below does not suit you

**What there is:**
- **The branch:** `claude/801-KRN-0246-the-shells-owner-client-of-the-gateway` (`53ca073e24`), from `main` `d9753fc2`.
- **The constant:** `common/abi.rs` has `SLOT_EFFECTOR` 45. The slot is the shell's alone and not a launch slot.
- **The ABI:** `SLOT_DYNAMIC` is 46 and the ABI is 33; the handoff stays 35.
- **The grant:** at each start of the shell, `init` lends there `effector_gw`'s client badged `mind::effector::BADGE_OWNER` (2).

**One difference from what you asked.** You asked for no client while no mode is on, and a new one at each start of the gateway. `init` keeps the gateway's endpoint across its starts, as it keeps every service's, so:
- **one client is enough:** the one the shell gets at its start reaches every later start of the gateway;
- **without a mode:** no task serves the endpoint, and a call answers `ERR_PEER`.

The shell can read `ERR_PEER` as "the gateway does not run" (no mode is on), as it already does for the camera's prepared gateway. Nothing is lent into a running shell.

**Tested:** the modes suite checks:
- with a mode on, only the shell holds badge 2 of the gateway's endpoint, apart from `init` and the gateway itself, and it holds it in slot 45;
- without a mode, slot 45 holds an endpoint that no task serves.

It is running now on x86 and aarch64, with the host tests. Your `effector status`, `audit` and `reload-policy` after `allow` can use the slot as soon as this lands.
