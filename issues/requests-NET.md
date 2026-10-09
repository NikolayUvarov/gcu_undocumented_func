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

## HTTP for a service, a certificate pin and long-lived sessions (501, 700)

**Recorded by:** the maintainer's session for main task [501](501-effector.md), and the Effector agent track (EFF) for main task [700](700-effector-agent.md), both on 2026-10-09 at the maintainer's request; merged into one request the same day.

### Problem

The agent of 700, which 501's tests also use, talks to Effector, a server that already exists.

**What Effector's protocol needs:**
- it posts JSON with `Authorization: Bearer` (a heartbeat every second, acknowledgements, operation results);
- it polls with GET and holds a wake-up stream open for up to 55 s;
- it uploads files of up to 1 GiB with an exact `Content-Length`;
- it downloads with `Range`.

**What `libmind::http` lacks.** It (351-NET-0001; its head parsed by `parse` since 109) makes one GET per connection with fixed headers and `Connection: close`. It has no POST and refuses a chunked body. A Go server sends a response without `Content-Length` as chunked once the response outgrows its buffer. A new TLS handshake every second would also be costly on the target.

**The pin.** Effector's agents pin the server by the SHA-256 of its leaf certificate (DER), with a second pin for rotation, and still check the certificate's SAN against the host name or address. [351-NET-0002](351-NET-0002-https-for-programs.md) pins the server's public key (SPKI): a different value. On the network track's branch, `tls.wit` 1.1 `connect-pinned` takes one SPKI pin and checks no name. Effector is asked (its task 75) to publish SPKI pins too, so the agent can start with `connect-pinned` as it is; the second pin is still needed for a key change. The `tls` service also allows 8 sessions in the system and a 5-second idle timeout inside `send` and `close`, while the agent keeps two sessions open for hours.

### Plan (a proposal; the network track decides)

- **`libmind::http` requests:**
  - methods `GET`, `POST` and `PUT`;
  - caller headers within a bound, with `Host`, `Content-Length` and `Connection` kept by the library;
  - a request body streamed from a caller source.
- **`libmind::http` responses:**
  - `Content-Length` or chunked, with chunk-size and trailer bounds;
  - close-delimited bodies when the caller allows them;
  - a response body read in pieces with an idle timeout chosen by the caller (the stream);
  - heads still through `parse`;
  - no redirects.
- **Kept-alive connections:** several requests over one transport.
- **The size limits** are stated.
- **`tls`:** a `connect` that accepts the server by one or two pins, so a pin can rotate.
  - **Two kinds of pin:**
    - the SHA-256 of the leaf certificate, as Effector's agents use;
    - 351-NET-0002's SPKI pin.
  - **One minor version** of `idl/tls.wit` serves both 351-NET-0002 and this.
  - **Checks kept** with a pin: the SAN, the validity, the server-authentication purpose and `CA:false`.
  - **TLS 1.3 only, as now.** Effector accepts TLS 1.2 and later.
- **Long-lived sessions:** a session that stays open between requests, and reads that wait without closing it.

### Acceptance criteria

- **Over one TLS session** to the 700-EFF-0002 test server, in QEMU, a service:
  - posts a JSON body and reads a chunked response, then 100 more requests;
  - holds a 55-second stream to its end.
- **Pins:** a server that matches neither pin is refused; one that matches the next pin is accepted.
- **Names and dates:** a wrong name and an expired certificate are refused.
- **An idle session:** a session left without traffic for ten minutes is then used.

## JSON in the parser service (700, 501)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [700](700-effector-agent.md) at the maintainer's request.

### Problem

Effector's commands and replies are JSON. Under MC-11.11 and the authority map ([docs/network/airlock.md](../docs/network/airlock.md)), the process that holds the server's token, the flow and the TLS client must not parse them itself. 109's `parse` service parses HTTP heads only. The updater's channel metadata is also JSON, and the map already plans to parse it in `parse`.

### Plan (a proposal; the network track decides)

- **A `json` call in `idl/parse.wit`** (a new minor version): bounded bytes in, a bounded typed tree out (objects, arrays, strings, integers, booleans, null), with stated limits on depth, members and string length.
- **The parser itself:** the `mind::json` library of [700-EFF-0004](700-EFF-0004-json.md), so the service and host tests share one implementation.
- **The client** still checks the typed tree against what it asked for (MC-11.5).

### Acceptance criteria

- `parse` turns a valid Effector command into its typed tree.
- It refuses malformed, too deep and too large input, and logs the refusal with the client's PID.
- The authority map gains the row for the agent's stage when 700-EFF-0006 lands.

## A flow for a long-running service (700)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [700](700-effector-agent.md) at the maintainer's request.

### Problem

`init` gives a boot service its flow once, and a grant keeps its term and volume until it ends: by default 3600 s and 16 MiB. The policy line can name longer ones (108). The Effector agent talks to one server for as long as the machine runs. Its heartbeats alone come to several megabytes an hour, and packages and files can reach 1 GiB.

### Plan (a proposal; the network track decides)

- **Bounds.** State the largest term and volume a policy line may give. The owner then sets the agent's line with `netpolicy add` (108), and nothing new is needed for a first version.
- **Later, if the track agrees:**
  - a flow the policy marks renewable, renewed by `netpolicy` before its term ends without closing sockets, while the policy still allows it;
  - a volume counted per period;
  - a host name resolved again when the service reconnects.
- **Init's part.** Any change to how `init` hands out the flow is a kernel task.

### Acceptance criteria

The policy guide states the bounds. In the `net` suite, a service whose line gives a long term keeps a connection past the default term. A renewable flow, if added, keeps a connection across its renewal.
