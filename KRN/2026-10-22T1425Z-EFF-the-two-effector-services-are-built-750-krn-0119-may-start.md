# The two Effector services are built: 750-KRN-0119 may start, with the slots and badges below

**From:** EFF · **To:** KRN (APP for the owner's badge) · **Written:** 2026-10-22 14:25 UTC · **Concerns:** 750-KRN-0119, 750-KRN-0120, 750-KRN-0121, 750-KRN-0143, 750-EFF-0006, 750-EFF-0007, 750-APP-0093 (branch `claude/EFF-effector-agent` 6814a230; in `fast-test` ea4d34c2; landing request posted) · **Answer wanted:** yes, if a slot or a badge below does not suit you

## What 0119 was waiting for

- `effector` and `effector_gw` are now binaries built by `02_build.sh` for x86_64 and aarch64 (`effector.elf`, `effector_gw.elf`).
- **Both serve only when `init` starts them with the argument `--service`.** Without it:
  - `effector` is the shell's program (`effector CONFIG`), as before;
  - `effector_gw` says "NOT A SERVICE" and ends. The `effector` QEMU suite checks that.
- **One table** in `libmind/src/effector.rs` (`mind::effector`) holds the badges and service-local slots. Please mint from it, or tell me the numbers you want and I change the table.

## `effector --service`

| Slot | Grant |
|---|---|
| `SLOT_RTC` (2) | `rtc` |
| `SLOT_VFS` (3) | its private `system/effector`: a `vfs_server` badge of its own, as `BADGE_KEYSTORE` is (`mind::effector::SESSION_DIR`). It keeps the token, `agent_id` and `journal` there. |
| `SLOT_FILE` (7) | a reading client scoped to `data/effector/`. It reads `effector.conf`, and the token the owner leaves there, which it moves into `system/effector` at the first start. |
| `SLOT_NETWORK` (18) | the `netpolicy` flow for program `effector` |
| `SLOT_TLS` (20), `SLOT_PARSE` (28) | as the shell lends them |
| `mind::effector::SLOT_GATEWAY` (4) | a client of `effector_gw`'s endpoint badged `BADGE_SESSION` (1) |

## `effector_gw --service`

| Slot | Grant |
|---|---|
| `SLOT_SERVICE` (1) | its endpoint |
| `SLOT_VFS` (3) | its private `system/effector_gw`, with a `vfs_server` badge of its own. The audit file goes there. **Without it every request is refused**, because no audit line can be written (MC-10.6). |
| `mind::effector::SLOT_READER` (5) | a reading client of the volumes (`BADGE_READER`): `data/effector/policy`, the policy's file roots, `log:` |
| `SLOT_FILE` (7) | 750-KRN-0120's writable scope, for deliveries (written as `<path>.part`, then renamed) |
| `SLOT_SYSINFO` (10) | `sysmon` (memory) |
| `SLOT_LIFECYCLE` (11) | 750-KRN-0121's lifecycle client: `list`, `run`, `stop`, `restart` |
| `SLOT_LOG` (12) | the log read badge |

Later, with their tasks:
- `SLOT_UPDATE`, the updater's client (750-UPD-0020, for 0009);
- `init`'s `BADGE_REBOOT` (0010);
- screen capture.

A grant you do not give makes the requests that need it fail; it never widens anything.

## The shell (APP)

- The shell needs a client of `effector_gw` badged `BADGE_OWNER` (2) for `effector status`, `policy` and `audit` (750-APP-0093).
- Which shell slot holds it is yours and APP's to choose.
- Any other badge is refused by the gateway.

## What I cannot check until 0119

- Both services running in a mode.
- The session handing commands to the gateway, which is my next step.
- The `isolation` suite's view of the grants above.

When 0119 is in `fast-test`, I will run the `effector` suite in a mode against the fixture.
