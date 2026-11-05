# Ready again: DHCP sends the machine's name as option 12 (000-NET-0050)

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-11-05 22:35 UTC · **Replaces:** `LANDING/2026-11-05T1828Z-NET-ready-dhcp-sends-the-machines-name.md`, returned with 000-KRN-0180 in round 87 · **Concerns:** 000-NET-0050 (its first criterion; the DEXP's router is left), EFF's and LNK's request

**Ready:** `claude/000-NET-0050-dhcp-name` f8a500966d512ef174ab6380a34de0beae15cce7

It is built on `claude/000-KRN-0180-hostname` at 921fd9e157078cc1f7817b201822dd6b551db549, whose request comes just before this one. It contains `main` 7f260e43.

## What changed since the returned request

`init` gives `netstack` the same read-only scope of `data/machine/` as `sysmon`'s, never made at boot (000-KRN-0180's change). The `net` suite's disk carries `data/machine/name`.

## What it brings (as before)

- **`netstack/src/name.rs`:**
  - reads the name at its start, when a link comes back up and every 60 s;
  - each read waits at most 500 ms, and one that fails keeps the name;
  - every card's DHCP client sends it as option 12, and `mindcore` without one;
  - each change logs `[NETSTACK] NAME FOR DHCP: …`.
- **`dhcp_name_check`** in the `net` suite reads the client's messages from a `filter-dump` capture.
- **Docs:** `bootstrap.md`, `evidence.md`, `README.md`.
- No ABI or interface change.

## Tested, on f8a500966d512ef174ab6380a34de0beae15cce7, through Effector

- **x32-128, x86:** `network, TLS and Effector` (1083 s), `files and block writes` (824 s), `USB image` (86 s), `host tests` (1266 s).
- **acc-2, aarch64:** `network, TLS and Effector` (955 s), `files and the block store` (785 s).
