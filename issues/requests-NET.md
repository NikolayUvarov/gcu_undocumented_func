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

## A TLS client without the device certificate (351)

**Recorded by:** the tools track (APP), 2026-10-09, while doing [351-APP-0028](../issues-done/351-APP-0028-shell-lends-its-tls-client.done).

### Problem

The shell now lends its client of `tls` to a program that asks for `REQUEST_TLS` and gets a flow grant. With that client, the program may call `connect` with `client-certificate`, and so present the device certificate to a server its grant reaches. Signing in as the device is more than a program needs to fetch a file over HTTPS.

### Plan (a proposal; the network track decides)

- A badge on the TLS client, for example `BADGE_DEVICE_CERTIFICATE`, without which `connect` refuses `client-certificate` (`denied`).
- The shell keeps the badged client for its own `https -c` and lends one without the badge.
- `init`'s grant to `updater` (351-KRN-0022) carries the badge only if the update server asks for the device's certificate.

### Acceptance criteria

The `tls` suite: a program with the lent client gets `denied` for `client-certificate`, while the shell's `https -c` still sends it.

## A list of the stack's sockets, for `netstat` (000-APP-0031)

**Recorded by:** the tools track (APP), 2026-10-09, for [000-APP-0031](000-APP-0031-netstat.md) (the tools plan's phase T4: `netstat`, after track D).

### Problem

`socket.wit` can name the interfaces (`interfaces`) and a grant's totals (`policy-usage`), and `netpolicy.wit` the live grants (`list`). Nothing names the stack's sockets. A `netstat` cannot show which connections are open, in what state, to where, or for which program.

### Plan (a proposal; the network track decides)

- `sockets: func(start: u32) -> list<socket-info, 32>` in `socket.wit` (a new minor version), page by page.
- `record socket-info { id: u32, protocol: protocol, local-port: u16, remote-address: u32, remote-port: u16, state: u8, badge: u16, sent: u64, received: u64 }`, where `state` is the TCP state (listen, syn-sent, established, fin-wait, close-wait, time-wait, closed) and `badge` the grant the socket belongs to.
- Who may call it is the network track's choice. One way: only an unbadged client, such as the shell's, may call it. A program would then never see other programs' flows. `netstat` then runs in the shell, which maps each badge to its program through `netpolicy.list`.

### Acceptance criteria

In QEMU, while `download` fetches from the test server, the list shows its TCP connection as established, with the server's address and port and the badge of `download`'s grant; after it ends, the socket is gone or closed.
