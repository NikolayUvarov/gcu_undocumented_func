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

**Recorded by:** the tools track (APP), 2026-10-09, while doing the same work as [351-APP-0017](../issues-done/351-APP-0017-shell-lends-the-tls-client.done).

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

## The aarch64 boot-slot check finds a dirty volume since the device key is stored at boot (351-NET-0005)

**Recorded by:** the tools track (APP), 2026-10-09, while gating its branch locally.

### Problem

`tests/aarch64_smoke.py` fails at `boot_slots_check.fsck` (351-UPD-0006) on `origin/main` (ae80444) run on this session's machine (QEMU 8.2, 4 CPUs). It passed on GitHub's runners for the tools branch merged with the same main (run 37909196366), so it depends on timing. Its four boot checks pass, then:

```
Reclaimed 2 unused clusters (4096 bytes).
Dirty bit is set. Fs was not properly unmounted and some data may be corrupt.
```

On aarch64 the processor has RNDR, so at every boot of a fresh image `keystore` makes the device key and stores it in `system/keystore/`. The check closes the machine soon after `MIND CORE KERNEL: INIT STARTED`, in the middle of that write. vfs_server has marked the volume dirty and allocated clusters it has not linked yet. On x86 the default processor has no RDRAND, nothing is written, and the same check passes.

On a vvfat boot disk, the same write once made QEMU itself stop: `block/vvfat.c:2760: handle_renames_and_mkdirs: Assertion 'j < s->mapping.next' failed` (the aarch64 general boot, same run).

### Plan (a proposal; the owning tracks decide)

The check could wait for `[INIT] READY` and for keystore's line before closing the machine, or boot without RNDR. Alternatively, keystore's first write could happen where a test that kills the machine early does not meet it.

### Acceptance criteria

`python3 tests/aarch64_smoke.py` passes on main.

## A read-only client of a card's counters, for `wm`'s background (000-APP-0049)

**Recorded by:** the tools track (APP), 2026-10-10, for [000-APP-0049](000-APP-0049-network-traffic-on-wms-background.md), split from 000-APP-0047 (the maintainer's request: the network traffic on `wm`'s desktop background).

### Problem

`wm` can show the network traffic on its desktop, but a card's counters (`net.wit` `counters`: sent, received, dropped) are reached only through a full client of the driver, which can also send and receive frames. The shell holds that client for `net`; lending it to `wm` would give `wm` the card itself.

### Plan (a proposal; the network track decides)

- A badge of the driver's endpoint that answers `info` and `counters` and refuses the rest (`net.wit` 1.x, with its version and transition), minted by whoever holds the full client.
- The shell lends it to a program that asks for it (a request flag for it comes with the tools track's change; a fixed slot, if one is needed, from the kernel track).

### Acceptance criteria

A program holding the read-only client reads the counters and is refused `send`, `receive` and `attach`.

## HTTP for a service, a certificate pin and long-lived sessions (501, 750)

**Recorded by:** the maintainer's session for main task [501](501-effector.md), and the Effector agent track (EFF) for main task [750](750-effector-agent.md), both on 2026-10-09 at the maintainer's request; merged into one request the same day.

### Problem

The agent of 750, which 501's tests also use, talks to Effector, a server that already exists.

**What Effector's protocol needs:**
- it posts JSON with `Authorization: Bearer` (a heartbeat every second, acknowledgements, operation results);
- it polls with GET and holds a wake-up stream open for up to 55 s;
- it uploads files of up to 1 GiB with an exact `Content-Length`;
- it downloads with `Range`.

**What `libmind::http` lacks.** It (351-NET-0001; its head parsed by `parse` since 109) makes one GET per connection with fixed headers and `Connection: close`. It has no POST and refuses a chunked body. A Go server sends a response without `Content-Length` as chunked once the response outgrows its buffer. A new TLS handshake every second would also be costly on the target.

**The pin.** Effector's agents pin the server by the SHA-256 of its leaf certificate (DER), with a second pin for rotation, and still check the certificate's SAN against the host name or address. [351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done) pins the server's public key (SPKI): a different value. `tls.wit` 1.1 `connect-pinned` takes one SPKI pin and checks no name, root or validity period. Effector is asked (its task 75) to publish SPKI pins too, so the agent can start with `connect-pinned` as it is; the second pin is still needed for a key change. The `tls` service also allows 8 sessions in the system and a 5-second idle timeout inside `send` and `close`, while the agent keeps two sessions open for hours.

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
  - **One minor version** of `idl/tls.wit` (1.2) adds both to 1.1's `connect-pinned`.
  - **Checks kept** with a pin: the SAN, the validity, the server-authentication purpose and `CA:false`.
  - **TLS 1.3 only, as now.** Effector accepts TLS 1.2 and later.
- **Long-lived sessions:** a session that stays open between requests, and reads that wait without closing it.

### Acceptance criteria

- **Over one TLS session** to the 750-EFF-0002 test server, in QEMU, a service:
  - posts a JSON body and reads a chunked response, then 100 more requests;
  - holds a 55-second stream to its end.
- **Pins:** a server that matches neither pin is refused; one that matches the next pin is accepted.
- **Names and dates:** a wrong name and an expired certificate are refused.
- **An idle session:** a session left without traffic for ten minutes is then used.

## JSON in the parser service (750, 501)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [750](750-effector-agent.md) at the maintainer's request.

### Problem

Effector's commands and replies are JSON. Under MC-11.11 and the authority map ([docs/network/airlock.md](../docs/network/airlock.md)), the process that holds the server's token, the flow and the TLS client must not parse them itself. 109's `parse` service parses HTTP heads only. The updater's channel metadata is also JSON, and the map already plans to parse it in `parse`.

### Plan (a proposal; the network track decides)

- **A `json` call in `idl/parse.wit`** (a new minor version): bounded bytes in, a bounded typed tree out (objects, arrays, strings, integers, booleans, null), with stated limits on depth, members and string length.
- **The parser itself:** the `mind::json` library of [750-EFF-0004](750-EFF-0004-json.md), so the service and host tests share one implementation.
- **The client** still checks the typed tree against what it asked for (MC-11.5).

### Acceptance criteria

- `parse` turns a valid Effector command into its typed tree.
- It refuses malformed, too deep and too large input, and logs the refusal with the client's PID.
- The authority map gains the row for the agent's stage when 750-EFF-0006 lands.

## A flow for a long-running service (750)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [750](750-effector-agent.md) at the maintainer's request.

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

## The raw-frame interface between `bcm_wifi` and the station (550-DRV-0027)

**Recorded by:** the drivers track (`DRV`), 2026-10-12, for the station of "An 802.11 station and WPA2-PSK" above.

### Problem

On the MacBook Pro, `bcm_wifi` now receives 802.11 frames by DMA: a scan listed five networks around with their SSID, BSSID, channel, signal and security ([550-DRV-0025](../issues-done/550-DRV-0025-bcm4331-receives-frames.done)). Transmitting is next ([550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done)). The station needs an interface to the driver, which 550-DRV-0006 says the two tracks agree on.

### Plan (a proposal; the network track decides)

`idl/wlanhw.wit` 1.0, served by `bcm_wifi`, used by `wlan` only:
- `info` → MAC address, supported channels;
- `set-channel(channel)`; `set-filter(all-beacons, bssid, own-address)`;
- `send(frame, rate)` → transmit status (acknowledged or not, retries);
- `receive` → frames from a shared buffer, each with signal (dBm), channel, rate and the FCS already checked;
- later, `set-key`/`clear-key` for the chip's CCMP, only if the station wants it.

The driver keeps no 802.11 state beyond the filter and the channel: authentication, association, scanning policy, keys and rates are the station's.

### Acceptance criteria

`NET` agrees the interface (or changes it) before `DRV` builds it; the driver serves it on the MacBook Pro (550-DRV-0027).

## A notice: 109-NET-0010 is done by the kernel track (109)

**Recorded by:** the kernel track (KRN), 2026-10-12, at the maintainer's request ("109-NET-0010 задачу ожидаем исполнения от тебя").

### What was done

The kernel session built both parts on its branch `claude/youthful-mendel-mf1soy`, and they reach `main` through the gate:

- **109-KRN-0096 (the loader starts a helper):** `idl/loader.wit` 1.8 gains `helper(parse, served)`. The loader starts a copy of `parse` whose only capability is the caller's endpoint, with a bounded quota and a per-owner limit.
- **109-NET-0010 itself:** `parse --session` answers on that endpoint alone and ends when its client does or after 30 s idle; `mind::parse::Session` owns such a copy, and `download` uses one, falling back to the shared service. The `net` suite's `session_parsers_check` shows two downloads with a parser each.

### What is asked of the network track

- **Do not build it again.** The request "A program started with no standard clients, for a parser per session (109-NET-0010)" in `issues/requests-KRN.md` on the network branch is covered by 109-KRN-0096: drop it when merging `main`.
- **Review and own.** The parser and `download` are the network track's: revise them as the track sees fit. The updater's checks on its own parser wait in `issues/requests-UPD.md`.

## `idl/wifi.wit`: what `wifi` needs of the station (550-APP-0033)

**Recorded by:** the tools track (APP), 2026-10-12, at the maintainer's word ("do the tools track's part of the network and Wi-Fi configuration"), for the Wi-Fi configuration interface that "An 802.11 station and WPA2-PSK" above proposes.

### Problem

`wifi` (550-APP-0033) is built against a stand-in station (`wifi/src/station.rs`, `wifi --demo`), since the station does not exist. Its calls are what the tools track asks the station's configuration interface for. The network track decides the interface; the program follows it.

### Plan (a proposal; the network track decides)

```wit
/// The 802.11 station's configuration (`wlan`): the access points around, joining one, the link, the networks it
/// remembers. The setup program and the shell hold the badge that changes settings; others may only read.
package mind:wifi@1.0.0;

interface wifi {
    enum security { open, wep, wpa2, wpa3, enterprise }
    enum error { no-device, busy, invalid, denied, unsupported }
    enum state { off, joining, joined, failed }
    enum reason { none, wrong-passphrase, no-answer, refused, no-address, lost }
    /// An access point the last scan heard: its network's name (empty: hidden), BSSID in the low 48 bits, channel,
    /// signal as minus dBm (48: -48 dBm; the IDL has no signed integers), protection, and whether a passphrase for
    /// the name is kept.
    record network { ssid: string<32>, bssid: u64, channel: u8, weakness: u8, security: security, remembered: bool }
    /// The link: its state, the network's name, the IPv4 address once DHCP answered (0 before), the signal, and why
    /// joining failed.
    record status { state: state, ssid: string<32>, address: u32, weakness: u8, reason: reason }
    /// The access points of the last scan; a new scan starts.
    scan: func() -> result<list<network, 32>, error>;
    /// Starts joining `ssid` (`status` follows): `passphrase` empty for an open or a remembered network; `remember`
    /// keeps it through `keystore`, never in a file (MC-11.9). Change badge only.
    join: func(ssid: string<32>, passphrase: string<64>, remember: bool) -> result<_, error>;
    status: func() -> status;
    /// Leaves the network (change badge only).
    leave: func() -> result<_, error>;
    /// Forgets a remembered network; false when none had the name (change badge only).
    forget: func(ssid: string<32>) -> result<bool, error>;
}
```

- `wifi` checks a passphrase before it sends it (WPA2-PSK: 8 to 63 printable characters or 64 hexadecimal digits); the station checks it again.
- The passphrase crosses the interface once, to the station, which hands it to `keystore`; it is never answered back.
- A slot and a request bit for the client are asked of `KRN` (requests-KRN.md, "A slot for the Wi-Fi station's configuration client").

### Acceptance criteria

The network track agrees or changes the interface; `wifi` follows it in a new step of 550-APP-0033, and its QEMU check runs against a stand-in service of the agreed interface.

## The stack's third card in slot 4 (550-KRN-0097, for 550-NET-0018)

**Recorded by:** the kernel track (KRN), 2026-10-12, answering "The Wi-Fi station in the boot services" (requests-KRN.md on the network branch).

### What the kernel track prepared

550-KRN-0097, built and passing the `net` suite on the network branch's 40ba302 merged with the kernel branch; it reaches `main` once 550-NET-0017 and 550-NET-0018 are there:

- `init` starts `wlansim` (where a test puts it) and `wlan` when a radio runs, and lends the radio's client to `wlan` alone, in slot 2;
- `netstack` gets `wlan`'s client without a badge in **slot 4**;
- the shell gets `wlan`'s client with `BADGE_WIFI_CHANGE` in `SLOT_WIFI`: 33, a launch slot, so `SLOT_DYNAMIC` moves to 34 (ABI 9), with `REQUEST_WIFI` = `1 << 22`;
- the `net` suite puts the `wlansim` fixture on its disk as `wlansim.elf` (`--wlansim-elf`, or where `scripts/x86_fixtures.sh` and `build_aarch64.sh --fixtures` build it) and checks these grants with `stat caps`.

### What is asked of the network track

- **`netstack`:** read slot 4 as its third card (`CARDS` = 3). Slot 4 is empty without the station, as slot 3 is without a second card.
- **The `net` suite:** with the station up, `netstack` finds three cards, and a join to `MIND-TEST` gets 10.0.3.15.
