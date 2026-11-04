# 111-NET-0057 — `timed` sets the calendar clock by SNTP within bounds (111's stage 1)

**Type:** network (a client service, `idl/socket.wit`) · **Owner:** `NET` (the kernel session) · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [111](111-network-time.md) · **Roadmap:** track D (network) · **Constitution:** MC-5.6, MC-9.4, MC-10.3, MC-11.4, MC-11.6, MC-11.11, MC-12.3, MC-12.4

## Problem

Nothing sets the calendar clock but the user (111's problem). This task makes 111's stage 1, the service and its bounds. Starting it at boot with its grants is [111-KRN-0206](111-KRN-0206-timed-among-the-boot-services.md).

## Plan

- **`mind::sntp`** (`libmind/src/sntp.rs`), pure and host-tested:
  - the request, with an unpredictable nonce as its transmit time;
  - the answer checked: its length, mode 4, version 3 or 4, not unsynchronised, stratum 1 to 15, the origin time the request's nonce, and non-zero times;
  - each answer's offset and round trip;
  - the choice: the largest set of at least two answers that all agree within 1 s beyond half their round trips, and its median. No choice when two such sets of the same size differ;
  - the bounds: never before the floor, nor past 2099; a step of more than 24 hours only at the first set since boot.
- **`socket.wit` 2.10, `grant-rules`:** a grant reads its own rules. `timed` learns its servers from them: the policy's lines `timed <server> udp <port>` are its time servers. The plan said port 123, and a line names it. A test names the ports its servers on the host listen on, since 123 needs root there.
- **`timed`:**
  - at most 4 servers asked at once, on one socket, with 2 s for the answers;
  - the clock read as its second turns, so that the offsets are right to a few milliseconds;
  - the clock set as the new time's second turns;
  - again every 6 hours, and every 10 s until the first set;
  - the log: each set with the old and new time and how many agreed, each answer's offset and round trip and whether it agreed, each refusal with its reason.
- **The floor:**
  - the commit time of the build (`MIND_SOURCE_TIME`, from `02_build.sh`). The release's manifest carries no build time, and the commit's time keeps the build reproducible;
  - the latest time `timed` set, in its private directory `system/timed/floor` (111-KRN-0206), which no other badge opens.

## Acceptance criteria

- **Host tests** (`tests/sntp_host.rs`): the codec, every field that makes an answer wrong, the choice, the floor, the step bound, and the rtc service's day and second.
- **Seeded fuzzing** of the answer's parser and of the choice, in the same file (`MIND_FUZZ_SEED`, `MIND_FUZZ_ITERATIONS`).
- **QEMU, x86 and aarch64,** the `net` suite's `timed_check`, against SNTP servers on the host behind QEMU's user network:
  - the clock set from 2024 to the host's time;
  - one server an hour ahead of two others left out;
  - two servers agreeing on 2020, before the floor, refused;
  - not started with no time line in the policy;
  - a script's `sleep(25000)` across the step lasting 25 s.
- **The profile** (`docs/profile/clocks.md`) says what the network time is trusted for, and its bounds.

## Related

[111](111-network-time.md), [111-KRN-0206](111-KRN-0206-timed-among-the-boot-services.md), [211-KRN-0051](211-KRN-0051-setting-the-clock.md).
