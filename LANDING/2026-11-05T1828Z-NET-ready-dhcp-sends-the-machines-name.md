# Ready: DHCP sends the machine's name as option 12 (000-NET-0050)

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-11-05 18:28 UTC · **Concerns:** 000-NET-0050 (its first criterion; the DEXP's router is left), EFF's and LNK's request

**Ready:** `claude/000-NET-0050-dhcp-name` 3a71a35a54f379233e89e058a5d68483e6d855da

It is built on `claude/000-KRN-0180-hostname` (f99b105e), whose request is in the queue before this one, and contains `main` 6253e7a7.

## What it brings

- **`netstack`** (`netstack/src/name.rs`):
  - reads `data/machine/name` through `init`'s read-only scope of `data/machine/` in `SLOT_FILE`, with `mind::hostname`'s rule;
  - reads it at its start, when a link comes back up and every 60 s;
  - a read waits at most 500 ms, and one that fails keeps the name;
  - every card's DHCP client sends the name as option 12 in DISCOVER, REQUEST and renewals, and `mindcore` without one;
  - each change logs `[NETSTACK] NAME FOR DHCP: <name> (<source>)`.
- **`init`:** `scopes_of("netstack")`, made as `sysmon`'s is.
- **The check:** `dhcp_name_check` in the `net` suite reads the client's messages from a QEMU `filter-dump` capture.
- **Docs:** `docs/profile/bootstrap.md` (`netstack`), `evidence.md` and `README.md`.
- No change to the ABI or an interface.

## Tested, `network, TLS and Effector`

- **x32-128, x86,** on 782c2830: passed (1087 s).
- **acc-1, aarch64,** on f4810b61: passed (936 s). The later commits are 0180's test and text changes and the issue's.
- **Host:** `sysmon`, `idl`, `services`.

**Seen on the way:** one x86 run failed in `wifi_join_check` after `dhcp_name_check` had passed. `wifi channels 00` printed no line on the console, though its log held the line; it passed on the next run. That is recorded in the issue.
