# 550-NET-0016 — WPA2-PSK as a library for the station

**Type:** network (library) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-11.9, MC-12.2, Appendix B.6

The first part of the drivers track's request in requests-NET.md ("An 802.11 station and WPA2-PSK", 2026-10-08): the keys, the handshakes and CCMP, before the station service (`wlan`) that uses them.

## Problem

The MacBook Pro's BCM4331 is a SoftMAC chip, so the host runs the 802.11 MAC layer and its security. No 802.11 code exists. WPA2-PSK needs:
- SHA-1 (HMAC, PBKDF2);
- AES key wrap;
- AES-CCM.

`tls` has none of these.

## Plan

A crate `wpa/`, with no libmind and no allocation, so `cargo test` runs it on the host and `wlan` links it on both targets:
- **`keys`:**
  - the PMK from a passphrase (PBKDF2-HMAC-SHA1, 4096 rounds) or from 64 hexadecimal digits;
  - the PRF of 12.7.1.2;
  - the PTK of CCMP-128 with AKM 2.
- **`eapol`:** EAPOL-Key frames of descriptor type 2, version 2:
  - read and written, with their MIC set and checked in constant time;
  - key data unwrapped (RFC 3394);
  - the RSN element and the GTK KDE found.
- **`handshake`:** the supplicant's 4-way handshake and group key handshake.
  - It checks every rule 12.7.6 and 12.7.7 give it: Ack, the replay counter, the same ANonce, the MIC, the encrypted key data, and the RSN element in message 3 equal to the advertised one (otherwise `Downgrade`).
  - It hands each key out once: a retransmitted message 3 or group message is answered, but its key is not installed again (key reinstallation).
- **`ccmp`:** CCMP-128 encapsulation and decapsulation:
  - the AAD and nonce of 12.5.3.3, for data and management frames, QoS and four-address frames;
  - a replay counter for each TID.
- The crates (`sha1`, `pbkdf2`, `aes-kw`, `ccm`, from the RustCrypto generation `tls` uses) in THIRD_PARTY.md. `sha1` and `aes` use their portable code, as `tls` does: the targets have no SSE in their ABI.

## Acceptance criteria

`wpa/tests/vectors.rs`, run by `scripts/host_tests.sh`, passes:
- **The standard's vectors:**
  - IEEE 802.11 J.4 (the PMK of three passphrases);
  - J.3 (PRF);
  - M.6.4 (CCMP).
- **RFC 3394 4.1** (key unwrap).
- **The PTK,** checked against Python's `hashlib` and `hmac`.
- **Both handshakes against an authenticator written in the test.** Every refusal is checked: a replay, a bad MIC, another ANonce, a downgrade, a missing bit, truncated frames, and keys not handed out twice.

The crate builds for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat`.

What this does not show:
- **A handshake with a real access point:** that comes with the station on the MacBook Pro.
- **The authenticator in the test** is written from the same reading of the standard as the supplicant, so a shared misreading would pass. The PTK and CCMP checks against independent implementations narrow that.

## Related

[requests-NET.md](requests-NET.md) ("An 802.11 station and WPA2-PSK"), [550-NET-0012](../issues-done/550-NET-0012-the-radio-interface.done) (`idl/wlanhw.wit`), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md).
