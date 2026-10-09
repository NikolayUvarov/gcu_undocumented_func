# Requests for the network track (NET), not numbered yet

**Owner:** network track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-08, for main task [550](550-network-on-real-hardware.md) at the maintainer's request

The network track numbers its own tasks (`NNN-NET-MMMM`), so requests from other tracks wait here. The network track turns each into a task and removes it from this file, and the file goes when it is empty.

## Several network interfaces (550)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [550](550-network-on-real-hardware.md) at the maintainer's request.

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

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [550](550-network-on-real-hardware.md) at the maintainer's request.

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

## An HTTP/1.1 client for a service (700)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [700](700-effector-agent.md) at the maintainer's request.

### Problem

`mind::http` (351-NET-0001) sends one GET with fixed headers and `Connection: close`. It refuses chunked bodies and bodies without `Content-Length`.

The Effector server needs more from its agents:
- `POST` with a JSON body and `Authorization: Bearer`;
- a response the server may send chunked, or close-delimited;
- a wake-up stream it holds open for up to 55 s;
- uploads of up to 1 GiB with an exact `Content-Length`;
- `Range` downloads.

Opening a TLS session for every request would mean a handshake every second, since the agent sends a heartbeat at that rate.

### Plan (a proposal; the network track decides)

- **Requests:**
  - `mind::http` methods `GET`, `POST` and `PUT`;
  - caller headers, within a bound, with `Host`, `Content-Length` and `Connection` kept by the library;
  - a request body from a caller `Source`, streamed.
- **Responses:**
  - `Transfer-Encoding: chunked`, with chunk-size and trailer bounds;
  - close-delimited bodies when the caller allows them;
  - the head up to the present 8 KiB.
- **Connections:**
  - HTTP/1.1 kept alive over one transport, so several requests share it;
  - the response body read in pieces with an idle timeout chosen by the caller (the stream).
- **Unchanged:** no redirects, still.
- **Host tests** in `tests/http_host.rs` for each of the above.

### Acceptance criteria

The host tests pass. In QEMU a program sends 100 `POST` requests over one kept-alive HTTPS connection to the 700-EFF-0002 test server, reads a chunked reply, and holds a 55-second stream to its end.

## A pinned leaf certificate and long-lived sessions in `tls` (700)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [700](700-effector-agent.md) at the maintainer's request.

### Problem

The Effector server identifies itself by a self-signed certificate. Its agents trust the SHA-256 of that exact leaf certificate (DER), with a current and a next value during rotation, and they still check that the host name or IP is in the certificate's SAN.

351-NET-0002 proposes a pin of the server's public key (SPKI). That is a different value, and a server cannot be pinned both ways by the same configuration.

The agent also keeps two sessions open for hours. The `tls` service's limits matter here: 8 sessions in the system, and a 5-second idle timeout inside `send` and `close`.

### Plan (a proposal; the network track decides)

- **The pin.** A `connect` with a set of up to two pins, each the SHA-256 of the leaf certificate's DER, instead of a chain to the roots, in the same new minor version of `idl/tls.wit` that 351-NET-0002 adds. Either the leaf-DER kind or the SPKI kind can be chosen per connection.
- **Kept with a pin:**
  - the SAN check against the name or address;
  - validity;
  - the server-authentication purpose;
  - `CA:false`.
- **TLS 1.3 only, as now.** The Effector server accepts TLS 1.2 and later, so 1.3 is enough.
- **Long-lived sessions.** A session that stays open between requests, and reads that wait without closing it.

### Acceptance criteria

In the `tls` suite:
- a server is accepted by its leaf pin, and by the next pin;
- it is refused with another certificate, with a wrong name, and with an expired certificate;
- a session stays open with no traffic for ten minutes and is then used.

## A flow for a long-running service (700)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [700](700-effector-agent.md) at the maintainer's request.

### Problem

`init` gives a boot service its flow once, and the policy's term and volume bound it: 3600 s and 16 MiB by default. When either runs out, the stack closes the grant's sockets.

The Effector agent talks to one server for as long as the machine runs. Its heartbeats alone come to several megabytes an hour, and packages and files can reach 1 GiB.

### Plan (a proposal; the network track decides)

- **Renewal.** A flow the policy marks renewable (`renew` on the policy line, proposed) is renewed by `netpolicy` before its term ends, without closing sockets, as long as the policy still allows it.
- **Volume per period.** A volume counted per period (for example, per hour) as well as in total.
- **A changed address.** A host name is resolved again when the service reconnects, and a changed address is allowed if the name still matches the policy.
- **Init's part.** Any change to how `init` hands out the flow is a kernel task.

### Acceptance criteria

In the `net` suite, a service with a renewable flow and a one-minute term keeps a connection for five minutes without a break. A flow that is not renewable still ends at its term.
