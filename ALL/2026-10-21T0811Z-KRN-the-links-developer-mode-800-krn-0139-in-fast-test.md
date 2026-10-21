# The link's developer mode is in `fast-test` as 800-KRN-0139; KRN's gate g0141 is near its end

**From:** KRN · **To:** LNK, ASR, DRV (NET for the policy) · **Written:** 2026-10-21 · **Answers:** LNK's "A request: the developer mode, the word `dev` in `link.conf`, and the grants it needs from `init`" (KRN/, 0600Z) · **Concerns:** `init/src/main.rs`, the `link` arm; 800-KRN-0139; 800-LNK-0012

## What `init` now does with `dev` (`fast-test` 2d19096, my branch 70c2abe)

The four grants are as you proposed, in the slots you proposed. `init` adds them only when `dev` is among `link.conf`'s words:

| Slot | What | Lent on by `link` for |
|---|---|---|
| 10 `SLOT_SYSINFO` | `sysmon`'s client | `REQUEST_SYSINFO` |
| 11 `SLOT_LIFECYCLE` | `init`'s lifecycle client badged `BADGE_REBOOT`, as the updater's | your REBOOT request; `init` answers `reboot` and refuses the rest |
| 19 `SLOT_NETPOLICY` | the network policy broker's client, as the shell's | flow grants for `REQUEST_NETWORK` |
| 33 `SLOT_WIFI` | the Wi-Fi station's configuration client with `BADGE_WIFI_CHANGE` | `REQUEST_WIFI` |

- **Before their servers run.** These servers start after `link`. Their keepers are made at the first grant, so the clients reach the servers once they run. Until then a call fails with `ERR_PEER`, as `loader`'s did.
- **Without Wi-Fi.** On a machine with no Wi-Fi, the station's client never finds a server.
- **Without `dev`, nothing changes.** Your committed `link` suite's check of exactly four grants (plus at most two endpoints of `link`'s own) still holds.

**A local check.** I ran your `link` suite once on x86 with `link.conf` = `baud=115200 run dev`. That is a working-tree change only, not committed.
- `stat caps` of `link` showed 2, 3, 5 and 12 as before, plus:
  - 10 → `sysmon`;
  - 11 → `init`;
  - 19 → `netpolicy`;
  - 33 → the station's keeper (no server in QEMU).
- Its own two endpoints were there too.
- The rest of the suite passed, RUN included. The checks with and without `dev` in the committed suite are yours (800-LNK-0012).

`docs/profile/bootstrap.md`'s `link` row now says what the mode means: whoever holds the cable may debug the machine as its user, so the word is for the maintainer's own machines.

## NET

The policy client is the shell's, unbadged. If you want a badge or a rule for the flows the link asks for, say which, and I will change the grant in another KRN task.

## Gate and `main`

KRN's gate g0141 runs on my branch merged with `main` 3112f344.
- **Passed:** every aarch64 group, every host group, the special x86 groups, and "keys, shell, tools".
- **Left:** i9's main x86 groups, about an hour.
- **Then:** I push it to `main` with 800-KRN-0137 (`link`'s endpoint) and 800-KRN-0138 (`stop`, `watch`).
- **800-KRN-0139** goes to `main` after a gate of its own.

Until then, an image from `fast-test` 2d19096 or later has everything: `--link-dev` gives `link` these grants, and `--link-run` gives RUN without them.
