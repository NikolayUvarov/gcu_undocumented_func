# Requests for the network track (NET), not numbered yet

**Owner:** network track · **Status:** open (the stack's third card in slot 4, which the kernel track prepared as 550-KRN-0097, is taken into [550-NET-0018](550-NET-0018-the-station-service.md), 2026-10-12; the raw-frame interface became [550-NET-0012](../issues-done/550-NET-0012-the-radio-interface.done), 2026-10-12; the list of the stack's sockets became [000-NET-0013](../issues-done/000-NET-0013-the-stacks-sockets.done), 2026-10-12; a read-only client of a card became [000-NET-0015](000-NET-0015-a-card-read-only-client.md), 2026-10-12; HTTP for a service and its pins became [750-NET-0019](750-NET-0019-http-for-services.md) and [750-NET-0020](750-NET-0020-tls-with-two-pins.md), JSON in the parser service [750-NET-0021](750-NET-0021-json-in-the-parser-service.md), a TLS client without the device certificate [351-NET-0022](351-NET-0022-tls-without-the-device-certificate.md), 2026-10-12; a flow for a long-running service became [750-NET-0014](../issues-done/750-NET-0014-the-bounds-of-a-flow.done), 2026-10-12; the aarch64 boot-slot check's dirty volume is gone since the test boots raw FAT images ([000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done)): `tests/aarch64_smoke.py` passed whole on `main` a4b3312, 2026-10-12) · **Recorded by:** the kernel track (KRN), 2026-10-08, for main task [550](550-network-on-real-hardware.md) at the maintainer's request

The network track numbers its own tasks (`NNN-NET-MMMM`), so requests from other tracks wait here. The network track turns each into a task and removes it from this file, and the file goes when it is empty.

## Several network interfaces (550)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [550](550-network-on-real-hardware.md) at the maintainer's request. **Progress:** a card's link going down and coming back is [550-NET-0023](550-NET-0023-a-cards-link-down-and-up.md) (2026-10-12); more than two cards wait for the station's card and the USB Ethernet driver, and `init`'s part for them is asked in requests-KRN.md ("The Wi-Fi station in the boot services").

### Problem

Since issue 105, `netstack` has one interface per card, but its cards are `virtio_net` instances only: two card clients, in slots 2 and 3, given by `init`. Real machines bring other drivers that serve the same `idl/net.wit` 1.2:

- a USB Ethernet adapter ([550-DRV-0005](550-DRV-0005-usb-ethernet.md));
- Wi-Fi, through the 802.11 station (next request);
- a tg3-family card ([550-DRV-0007](550-DRV-0007-broadcom-ethernet.md)).

A USB adapter or a Wi-Fi link comes and goes while the system runs. The default route is the first interface with a gateway, whatever its kind, and which card a flow may use is not part of the broker's grants (105, "not done").

### Plan (a proposal; the network track decides)

- Any driver that serves `idl/net.wit` 1.2 is a card: `virtio_net`, the USB Ethernet driver, the station; more than two of them.
- A card whose driver answers `no-device`, or whose link is down, has no routes. When its link comes up, DHCP runs again.
- The default interface follows a stated rule (for example, a wired link that is up before Wi-Fi), and the user can override it. `ip` shows the default and the kind of each card.
- Flows on a card that went away fail with a reason; new ones go out through the new default.
- Which card a flow may use, in `netpolicy`'s grants (105's remainder), if the track takes it now.
- `init`'s part, starting the drivers and handing their endpoints to `netstack`, is a kernel task.

### Acceptance criteria

In QEMU with `virtio_net` and the USB Ethernet driver on `usb-net`, both cards get leases. Removing the USB device through QMP moves the default to the VirtIO card; adding it back gives it a lease again. `ip` shows each step.

## An 802.11 station and WPA2-PSK (550)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [550](550-network-on-real-hardware.md) at the maintainer's request. **Progress:** the keys, the handshakes and CCMP are [550-NET-0016](../issues-done/550-NET-0016-wpa2-psk-library.done), the station's logic (scanning, joining, data) [550-NET-0017](../issues-done/550-NET-0017-the-stations-logic.done), the station service `wlan` with its configuration interface `idl/wifi.wit` 1.0 (agreed with the tools track) [550-NET-0018](550-NET-0018-the-station-service.md), and remembered networks kept by `keystore` [550-NET-0024](550-NET-0024-remembered-wi-fi-networks.md) (2026-10-12).

### Problem

The MacBook Pro's Wi-Fi is expected to be a Broadcom BCM4331, a SoftMAC chip ([550-DRV-0006](550-DRV-0006-broadcom-wifi.md)): the chip moves raw 802.11 frames, and the host runs the MAC layer. No 802.11 code exists. `tls` has AES and HMAC over SHA-2 from the RustCrypto crates; WPA2 also needs SHA-1 (HMAC and PBKDF2), AES key wrap and AES-CCM.

### Plan (a proposal; the network track decides)

- A station service (`wlan`, proposed name) between the Wi-Fi driver and `netstack`:
  - to `netstack` it is a card that serves `idl/net.wit`, turning Ethernet frames into 802.11 data frames and back;
  - to the driver it speaks the raw-frame interface of 550-DRV-0006, an IDL file the two tracks agree on.
- Scan, active and passive. It transmits only on the channels allowed where the device is, and scans passively until that is known.
- Open System authentication and association; rate control.
- WPA2-PSK:
  - the PMK from the passphrase (PBKDF2-HMAC-SHA1, 4096 rounds);
  - the 4-way handshake (EAPOL-Key, the PTK, the MIC, replay counters) and the group key handshake;
  - CCMP, in software, or in the chip when the driver offers it.
- WPA3-SAE and enterprise (802.1X) later; WEP and TKIP not at all.
- The passphrase or PMK of a remembered network is kept by `keystore`, which could derive each association's keys for the station without handing out the PMK (MC-11.9).
- A Wi-Fi configuration interface (`idl/wifi.wit`, proposed): scan results (SSID, BSSID, channel, signal, security), connect with a passphrase or to a remembered network, status, forget. Only clients with its badge may change anything (the setup program and the shell).
- The new crates' licences go into THIRD_PARTY.md.
- Tests: QEMU has no Wi-Fi device. Host tests check the handshake and CCMP against the test vectors in IEEE 802.11's annexes; the station is checked on hardware with 550-DRV-0006.

### Acceptance criteria

The host tests pass with the standard's vectors. On the MacBook Pro, with 550-DRV-0006, the station joins a WPA2-PSK network, `netstack` gets a lease, and the shell's `https` fetches a page.

## A notice: 109-NET-0010 is done by the kernel track (109)

**Recorded by:** the kernel track (KRN), 2026-10-12, at the maintainer's request ("109-NET-0010 задачу ожидаем исполнения от тебя").

### What was done

The kernel session built both parts on its branch `claude/youthful-mendel-mf1soy`, and they reach `main` through the gate:

- **109-KRN-0096 (the loader starts a helper):** `idl/loader.wit` 1.8 gains `helper(parse, served)`. The loader starts a copy of `parse` whose only capability is the caller's endpoint, with a bounded quota and a per-owner limit.
- **109-NET-0010 itself:** `parse --session` answers on that endpoint alone and ends when its client does or after 30 s idle; `mind::parse::Session` owns such a copy, and `download` uses one, falling back to the shared service. The `net` suite's `session_parsers_check` shows two downloads with a parser each.

### What is asked of the network track

- **Do not build it again.** The request "A program started with no standard clients, for a parser per session (109-NET-0010)" in `issues/requests-KRN.md` on the network branch is covered by 109-KRN-0096: drop it when merging `main`.
- **Review and own.** The parser and `download` are the network track's: revise them as the track sees fit. The updater's checks on its own parser wait in `issues/requests-UPD.md`.
