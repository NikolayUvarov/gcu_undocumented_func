# 111-NET-0075 — `timed` authenticates its time servers with NTS (RFC 8915)

**Type:** network (`timed`, `mind::nts`; `init`'s grant as a `KRN` part) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [111](111-network-time.md) (stage 2) · **Roadmap:** track D, track C · **Constitution:** MC-5.6, MC-9.4, MC-10.3, MC-11.4, MC-11.9, MC-11.11, MC-12.2, MC-12.3

## Problem

Stage 1's `timed` takes SNTP answers that anyone on the path can forge, within bounds (111-NET-0057). 111's stage 2 authenticates them with NTS: key establishment over TLS 1.3 (NTS-KE), then NTP packets that carry a unique identifier, a cookie and an authenticator sealed with AEAD_AES_SIV_CMAC_256.

## Plan

1. **The servers** come from the grant, as in stage 1:
   - an address with a TCP rule is an NTS server, its key establishment on that port (4460 when the rule names none), checked for the name the policy gave (`socket.wit` 2.12, 111-NET-0074), else for the address;
   - its NTP server is the one its key establishment names (an IPv4 address, else its own) on the port it names (else 123), and the grant must reach it by a UDP rule;
   - an address with a UDP rule and no TCP rule is a plain server, used as in stage 1. At most 4 servers in all.
2. **NTS-KE through `tls`** (`tls.wit` 1.4, 111-NET-0073): a session on `timed`'s own flow, `connect-alpn` with `ntske/1`, the request (NTPv4, AEAD_AES_SIV_CMAC_256), the response's records (`mind::nts`), and the C2S and S2C keys from `export`. `timed` never holds the TLS session's secrets (MC-11.9). `init` lends it a TLS client badged `BADGE_TLS_NO_DEVICE` when `tls` runs, and starts it right after `tls`.
3. **The NTP packets** (`mind::nts`, RustCrypto's `aes-siv`):
   - a request carries a fresh 32-byte identifier, one unused cookie, placeholders that bring the cookies back to 8, and the authenticator over the rest (a 16-byte nonce, so no additional padding);
   - an answer counts only when its identifier is the request's and its authenticator holds under the S2C key; SNTP's checks then apply to its header; the cookies it seals are kept.
   - An NTS NAK for this request drops the keys; a server out of cookies runs its key establishment again. A failed key establishment waits 10 s, twice as long at each failure, up to 6 hours.
4. **The bound:** when every server that agreed is authenticated by NTS, a step of more than 24 hours is taken at any set; the floor holds either way, and two servers must still agree.
5. **The log** (MC-10.3): each key establishment with its cookies and NTP server, or why it failed; each answer refused and why; each set with how many servers NTS authenticated.

## Acceptance criteria

- **Host:** `tests/nts_host.rs`: AES-SIV against RFC 5297's two examples; the NTS-KE request and the response's records taken and refused; the request's fields as a server reads them, its tag checked; an answer replayed (another identifier), forged (any byte), unprotected, sealing no cookie, an NTS NAK; the parsers on random and mutated input never panic.
- **QEMU, x86 and aarch64** (`tls` suite's `nts_check`, against NTS servers on the host behind QEMU's user network, named `mind.test` through the test's resolver):
  - the clock set from 2024 to the host's time by two NTS servers that agree;
  - a forged answer refused as not authentic, and an authentic answer to another request refused as not ours;
  - the cookies back to 8 after the answer;
  - a server that agrees on no application protocol, and one whose certificate no root vouches for, refused at NTS-KE.
- **Interop:** `timed` against an NTS implementation other than the test's (chrony).
- **The profile** (`docs/profile/clocks.md`) says what NTS time is trusted for, the bound and the circularity.

## Progress

**Made (2026-11-12)** on `claude/111-NET-0073-tls-alpn-exporter`, after 111-NET-0073 and 0074:
- `libmind/src/nts.rs` (`mind::nts`); `timed/src/siv.rs`; `timed`'s NTS servers, key establishment, requests, answers and the wider bound.
- `init` (the `KRN` part): `timed`'s TLS client (`BADGE_TLS_NO_DEVICE`, when `tls` runs), `timed` among `services::LATE` right after `tls` and in `BEFORE`; `tests/services_host.rs`. The boot images' order and the handoff are unchanged.
- `tests/qemu_smoke.py`: `_Nts` (NTS-KE over Python's `ssl`, the exporter from OpenSSL's key log, AES-SIV over `cryptography`'s AES-CMAC and AES-CTR, checked against RFC 5297 at each start), `nts_check` in the `tls` suite and alone as `--suites nts`.
- Docs: `docs/profile/clocks.md` (NTS), `docs/network/policy.md`, THIRD_PARTY.md (`aes-siv`, `cmac`, `dbl`).
- **Tested** (this container): `tests/nts_host.rs`, 6 tests, pass (3000 fuzzed inputs at the default seed); `tests/services_host.rs` passes; `timed` and `init` build for both targets.
- **Left:** the QEMU checks on both architectures; the interop with chrony; the gate.

## Related

- [111](111-network-time.md): the main task, stage 2.
- [111-NET-0073](111-NET-0073-tls-alpn-and-the-exporter.md): ALPN and the exporter in `tls`.
- [111-NET-0074](111-NET-0074-a-grants-host-names.md): the grant's host names.
- [111-NET-0057](../issues-done/111-NET-0057-timed-sets-the-clock-by-sntp-within-bounds.done): stage 1.
