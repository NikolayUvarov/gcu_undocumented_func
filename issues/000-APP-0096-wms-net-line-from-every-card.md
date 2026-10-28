# 000-APP-0096 — `wm`'s network line counts every card, the Wi-Fi station's too

**Type:** tools (`wm`, `shell`, interface) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open: in `main` since 2a33a71b (2026-10-20), the 250 ms bound since 1d841622 (2026-10-21), checked in QEMU; the MacBook Pro's run left · **Blocked by:** — · **Main task:** — (split from [000-APP-0049](000-APP-0049-network-traffic-on-wms-background.md)) · **Roadmap:** track D, G · **Constitution:** MC-3.11, MC-12.3, MC-12.4

**Recorded by:** the tools track, 2026-10-20, from the maintainer's report: "now we have network working but desktop widget with date time cpu load says 'no network counters'".

## Problem

- **`wm`'s `show = net` line reads one card** (000-APP-0049): the client of the first wired card's driver that `init` gives the shell in `SLOT_NET`, lent as a copy with `BADGE_COUNTERS`.
- **Over Wi-Fi it has nothing to read.** The station's card is the stack's third (`wlan`), and no client of it reaches the shell.
  - With no wired card (the MacBook Pro), the shell holds no `SLOT_NET` and lends nothing. `wm` says `NET: NO COUNTERS` while the network works.
  - With a wired card that carries nothing, it would show that card's traffic and not the Wi-Fi's.
- **The line does not say why.** `NET: NO COUNTERS` is all it shows, whether there is no card, no stack or no client.

## Plan

- **`idl/shell.wit` 1.7 `net-counters`:** every card's frames as `netstack` counts them (`socket.wit` `interfaces`), with its kind and whether it is the default (`cards`).
  - The shell answers it from its operator client of the stack.
  - It is for the window manager the shell started; `refused` for any other client.
  - It reads only and reaches nothing on the network (MC-3.11: the authority stays the shell's).
- **`wm` sums every card's frames** for `NET IN n/S OUT n/S`.
  - The shell's answer comes first. Without it, `wm` reads the card client lent with `BADGE_COUNTERS`, as before.
  - The shell is asked with a 250 ms bound. When it does not answer in time (busy with a long command), it is left alone for 5 s and the line stays as it was. (50 ms and 10 s at first: on PCU at a load of 11, gate 25, the shell never answered within 50 ms and `wm` gave no rate in 90 s.)
- **The line says why it has nothing:**
  - `NET: NO CARD`: the stack counts no card;
  - `NET: NO NETWORK STACK`;
  - `NET: NO COUNTERS`: neither source.
- **The first rate in `wm`'s log** names its source and the cards counted: `(EVERY CARD'S FRAMES, 2 CARDS, 1 WI-FI, THE FIRST RATE)`.

## Acceptance criteria

- **The `net` suite** (x86, aarch64): `wm` logs `[WM] NET IN … (EVERY CARD'S FRAMES, n CARDS, w WI-FI, THE FIRST RATE)`:
  - with the wired card: its frames, `0 WI-FI` (the station is the stack's card only once joined);
  - over Wi-Fi alone (the suite's last VM, no wired card, after `wifi join` with the stand-in radio `wlansim`): `1 CARDS, 1 WI-FI`, which the card client could not give.
- **The MacBook Pro over Wi-Fi:** the line moves while a download runs.

## Progress (2026-10-20)

- **`idl/shell.wit` 1.7** and `libmind/src/idl/shell.rs` (generated).
- **`shell/src/net.rs` `card_counters`;** the shell's `net-counters` answer for the window managers it started.
- **`wm/src/main.rs` `net_frames`:** the shell first, the card client second, the reason otherwise, the 10 s pause after a slow answer.
- **The `net` suite** reads the first rate's line with the cards counted, with the wired card and over Wi-Fi alone. The first gate run expected the station before its join: the stack lists it only once joined (`1 CARDS, 0 WI-FI` on i9 and un-rizen-service), so the Wi-Fi check moved after the join.
- **Docs:** `docs/tools` (EN, RU), `docs/idl`.
- **In `main`** since 2a33a71b (2026-10-20). The gate, on the branch with `main` abe3d70b:
  - i9 (x86): every group on a3ebd34c but RAM above 4 GiB and the padded vector area, which passed on 2a33a71b;
  - un-rizen-service (aarch64): build, boot and fault containment, programs, shell and four CPUs, NVMe and 16 CPUs on a3ebd34c; files, network and TLS, RAM, ACPI and PCI above 4 GiB, and GICv2 on 2a33a71b;
  - locally: the network driver without legacy, TLC, services, scheduling and files on a3ebd34c; the build, the host tests, boot and keys on 2a33a71b.
  The commits after a3ebd34c changed the suites, the IDL fuzzer's list and `dictate`'s and `speak`'s memory request; the groups that run them ran on 2a33a71b.
- **This is QEMU's virtio-net and stand-in radio**, not the MacBook Pro's Wi-Fi (MC-12.1).
- **A loaded machine** (gate 25, PCU at a load of about 11): the first rate over Wi-Fi did not come within the suite's 90 s twice, and passed alone. The bound is 250 ms and the suite waits 180 s (2026-10-21), in `main` with landing round 2, `main` 1d841622 (2026-10-21): the kernel session's gate of candidate 97c4a4c1.
- **Left:** the MacBook Pro over Wi-Fi: the line moves while a download runs.

## Progress (2026-10-21): the Mac says `NET: NO CARD`

- **The maintainer's report:** "we using wifi but i still see no card", on the MacBook Pro, its Wi-Fi working.
- **What the line means:** the shell answered `net-counters` with an empty list, which it gives when the network stack's `interfaces` lists no card.
- **What the link could read** (LNK, for ASR and DRV, 2026-10-21 19:00 UTC): `wm` did not run in the boot read. `netstack` had the station as card 2, with DHCP and the default route on it. `ip`, `netstat` and `ps` are the shell's commands, which the link does not reach.
- **For the next run:**
  - the shell logs an empty answer once: `[SHELL] NET-COUNTERS: THE STACK LISTS NO INTERFACE (<n> CARDS, <m> UP)`;
  - `wm` logs each reason for no rate once: `[WM] NET: NO CARD (THE BACKGROUND'S NET LINE)`.

  So the log the link reads says which side has no card.
- **Settings' `Net` row** said `(no counters yet)` from before this task. It now says `every card's frames a second, in and out`.

## Related

[000-APP-0049](000-APP-0049-network-traffic-on-wms-background.md) (the line, the card client), [000-NET-0015](000-NET-0015-a-card-read-only-client.md) (`BADGE_COUNTERS`), [550-NET-0018](../issues-done/550-NET-0018-the-station-service.done) (the station's card).
