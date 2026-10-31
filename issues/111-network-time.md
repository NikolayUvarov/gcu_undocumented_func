# 111 — Network time: the calendar clock set from time servers, authenticated with NTS

**Type:** network (main task: a client service) · **Owner:** `NET` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** — for stage 1; stage 2 by the `tls` service's exporter and ALPN · **Roadmap:** track D (network), track C (update: the channel's expiry rests on the clock) · **Constitution:** MC-5.6, MC-9.4, MC-10.3, MC-11.4, MC-11.6, MC-11.11, MC-12.3

**Recorded by:** the kernel session, 2026-10-31, at the maintainer's request: "who sets the time? An NTP client is needed, to take the time from time servers."

## Problem

- **Who sets the calendar time today.** Only the user, by hand.
  - The kernel keeps a monotonic clock only (`CLOCK`, `UPTIME`). Calendar time is the `rtc` service's (MC-5.6).
  - `rtc` reads the board's clock: the CMOS RTC on x86, the PL031 on aarch64. `rtc.wit` 1.2 `set` writes it, for a client with the setting badge. init gives that client to the shell alone ([211-KRN-0051](211-KRN-0051-setting-the-clock.md)).
  - The user's command `date set` is APP's part.
  - Nothing takes the time from the network: 211-KRN-0051 names NTP over `netstack` as later work, and [000-APP-0055](000-APP-0055-date-and-time-from-wms-settings.md)'s "now from the network" waits for it.
- **What a wrong clock breaks.**
  - The `tls` service checks a certificate's validity against the clock.
  - The updater refuses an expired channel, and counts a clock it cannot read as expired ([351-UPD-0025](../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done)).
  - Logs carry the calendar time. The MacBook Pro's log began `STARTED 2022-01-01` (211-KRN-0051).
- **Why plain NTP is not enough here.** An answer anyone on the path can forge lets them move the clock:
  - back, so that an expired channel or certificate reads as valid (a freeze, MC-9.4);
  - or forward, so that the device refuses every update and every certificate.

  So an unauthenticated time can be used only within bounds. Network Time Security (NTS, RFC 8915) authenticates the answers.

## Plan

### Stage 1: an SNTP client service, `timed`, within bounds

- **The service.** `timed`, a boot service of init, started only when the owner's policy holds a time line (`netpolicy.txt`: `timed <server> udp 123`, one line a server, at most 4).
  - Its authorities: a flow grant for those servers on UDP port 123, through `netpolicy` as any program's (MC-11.6), and its own `rtc` client with the setting badge (`rtc.wit` 1.2 `set`). It holds nothing else (MC-11.4).
  - SNTPv4 (RFC 4330 and RFC 5905's client mode): a request to each server, the answers parsed in `timed` alone, which holds no other authority (MC-11.11).
- **Which answer it takes.**
  - At least 2 servers that agree within 1 s, by their offsets and round trips (the intersection of their intervals); otherwise nothing is set, and it says so.
  - An answer whose stratum is 0 or above 15, whose leap indicator says unsynchronised, or whose origin time is not the request's, is refused.
- **The bounds** (the profile names them):
  - **Never earlier than a floor:** the build time of the booted release, from its manifest, and the latest time `timed` itself set, kept in `system/time/floor`. A time before either is refused and logged.
  - **A step of more than 24 hours** from the RTC's time is taken only at the first set after boot, or when the user confirms it. Smaller corrections are taken as they come.
- **When.** At boot once the network is up, then every 6 hours, and again when the user asks (`date sync`, APP's part).
- **The clocks stay apart** (MC-5.6): only the calendar clock moves. The kernel's monotonic clock, every deadline and every sleep are untouched by a correction.
- **The log** (MC-10.3): each set, with the old and new time, the servers that agreed and their offsets; each refusal with its reason.

### Stage 2: NTS (RFC 8915)

- **NTS-KE** over TLS 1.3 to port 4460, through the `tls` service: it needs ALPN (`ntske/1`) and the exporter (RFC 8446, section 7.5). Both are a `tls` task of this track. `timed` keeps the cookies, never the TLS keys.
- **The NTP packets** carry the NTS extension fields, sealed with AEAD_AES_SIV_CMAC_256.
- **The policy:** with NTS, the step bound may be wider. A server without NTS is used only when the owner's line says so.
- **The trust:** NTS-KE's certificate is checked against the image's roots (000-NET-0046), and against the clock. A device whose clock is before the certificate's start needs the floor to bootstrap. The profile states this circularity and its bound.

### Not in this task

Time zones for `date` and `wm` (APP's), and serving time to other machines.

## Acceptance criteria

- **Host tests:** the packet codec (every field, a wrong origin time refused), the choice of agreeing servers, the floor and the step bound.
- **A fuzz target** for the answer's parser (ASR's harness).
- **QEMU, x86 and aarch64,** against a test NTP server on the host behind QEMU's user network:
  - the clock set from wrong to right;
  - an answer before the floor refused;
  - one server disagreeing with two others ignored;
  - nothing set with no time line in the policy;
  - a correction leaving `sleep` and the deadlines of running programs unchanged.
- **Stage 2:** the same against an NTS server on the host; a forged or replayed packet refused.
- **The profile** (MC-5.6, MC-9.4, MC-12.3) says what the network time is trusted for, and the bounds.

## Related

[211-KRN-0051](211-KRN-0051-setting-the-clock.md) (`rtc.wit` 1.2 `set`), [000-APP-0055](000-APP-0055-date-and-time-from-wms-settings.md), [351-UPD-0025](../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done), [103](../issues-done/103-tls-service.done) (`tls`), [102](../issues-done/102-network-policy-broker.done) (flow grants), 000-NET-0046 (the image's roots).
