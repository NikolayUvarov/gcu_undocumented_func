# MIND IDL v0.2

**Version:** 0.2 (2026-10-04) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-2.3, MC-2.4, MC-2.12, MC-12.4, Appendix B.2 · **Roadmap:** [C5](../../ROADMAP.md)

MIND IDL describes the interface of a service: functions, the types and size limits of their data, the capability a call may carry and the version. Interfaces are written in a subset of [WIT](https://component-model.bytecodealliance.org/design/wit.html), the baseline candidate named in Appendix B.2. `scripts/mind_idl.py` generates Rust bindings for client and server into `libmind/src/idl/`. The generated files are committed, and `tests/idl_test.py` fails if they are out of date.

v0.2 covers the data representation and the acceptance of a single request: integers in two message words, and records, strings and lists in a memory buffer. Its minor extension (merged from the tools branch, issue [051](../../issues-done/051-merge-main-into-tools.done)) adds enums, `bytes<N>`, enum errors and capability results; it changes nothing on the wire for interfaces that do not use them. Message ordering, session state machines and composition (MC-2.10) are not part of it yet.

## Syntax

```wit
/// Documentation comments are copied into the bindings.
package mind:rtc@1.0.0;

interface rtc {
    /// Seconds since midnight; none if the RTC cannot be read.
    now: func() -> option<u32>;
}
```

- One `package <namespace>:<name>@<major>.<minor>.<patch>;` and one `interface` per file `idl/<name>.wit`. The major version is 1–255.
- Functions: `name: func(params) -> result;`. There are 1–255 functions per interface, numbered in declaration order starting at 1.
- Parameter types:
  - `bool`, `u8`, `u16`, `u32`, `u64`;
  - enums declared in the interface: `enum name { case, ... }` (1–256 distinct cases, numbered from 0);
  - `bytes<N>`: at most N bytes of binary data (1 ≤ N ≤ 65535), only as a parameter or a result, not in a record or a list;
  - `string<N>`: UTF-8 text of at most N bytes;
  - `list<T, N>`: at most N items; T is an integer, an enum, `string<N>` or a record;
  - records declared in the interface: `record name { field: type, ... }` (one or several lines), with the same field types;
  - at most one capability: `own<memory>`, `own<endpoint>` (moved, `CAP_TRANSFER_MOVE`) or `borrow<memory>`, `borrow<endpoint>` (copied, so the client can revoke it: a lease); `borrow<firmware>` is the firmware variable privilege (`CAP_KIND_FIRMWARE`, 351-KRN-0027), copied; `own<text>` is the writer of a program's text input (`CAP_KIND_TEXT`, 000-KRN-0094), as `loader.wit` 1.9 `commit-with-input` returns it.
- Result: none; any of the data types above; `option<T>`; `result<T, error-code>` or `result<_, error-code>` (a system error code, `ERR_*`); `result<T, E>` or `result<_, E>` with an enum `E` declared in the interface; in a word call also a capability, alone or as the `T` of a result: `own<…>` (the server gives it away) or `borrow<…>` (the server keeps its source and can revoke it, as `vfs.wit` `scope` does when the task it was made for ends); it arrives in a slot the client names.
- **Bounds are part of the type.** `string<N>` and `list<T, N>` are a MIND extension of WIT syntax: every value has a static maximum size, so every buffer is bounded.
- Anything else is rejected by the generator: resources, variants, unbounded strings or lists, several capabilities, a capability in a function that also uses a buffer (its capability is the buffer), a capability in a record or an option, more than 64 KiB per request or reply.

## Wire format

A function whose parameters and result are integers only (and at most one capability) is a **word call**: the two data words and the optional capability of an IPC `CALL`.

| Word | Request | Reply |
|---|---|---|
| 0 | bits 0–7 method, 8–15 major version, 16–63 fields | bits 0–7 status, 16–63 result |
| 1 | fields | result (if it does not fit in word 0), or the error code with status 2 |

Fields are placed in declaration order. A field never straddles a word boundary, so the size limit of a word call is 48 + 64 bits.

A word call waits for its reply. A client that calls a server which may be busy with the user, as `wm` calls the shell, runs the calls inside `mind::idl::wire::with_timeout(ms, ...)`: each gives up after `ms` with the system's timeout error, and a reply that comes later is discarded (211-APP-0044).

A function with a string, list or record anywhere is a **buffer call**:
- The client encodes the parameters into a buffer of its own, at least as large as the declared maxima of request and reply. It sends `method | major << 8 | request length << 16` in word 0 and 0 in word 1, with the buffer as a (copied) memory capability.
- The server writes the result into the same buffer and replies `status | reply length << 16`.
- Before reading the reply, the client revokes the server's copy, which unmaps it everywhere (MC-2.6). The server cannot change the reply while it is being decoded.

Encoding in the buffer, in declaration order, byte-packed, little-endian: integers in their size (`bool` one byte, 0 or 1), an enum as one byte, a string or `bytes<N>` as a u16 byte length and the bytes, a list as a u16 count and the items, a record as its fields. In a word call an enum is an 8-bit field.

A capability result travels as the capability of the reply: the client's call names the slot it is received in (the generated function takes `receive`), and the server moves or copies it with the reply. A reply that should carry one and does not is rejected by the client.

Status values:
- 0: ok;
- 1: `none` for an option result;
- 2: error of a `result<_, error-code>` (code in word 1) or of a `result<_, E>` with an enum (the case number in word 1, bits 8–63 of word 0 zero);
- 0x80: the request failed the receiver's schema check;
- 0x81: the receiver serves another major version.

## Receiver checks (MC-2.4, MC-2.11, MC-2.12)

The generated `decode(request, cap_slot)` returns the request and a `Call` that the reply functions consume. It accepts a request only if all of the following hold:
- it is a call;
- the major version matches;
- the method exists.

For a word call, in addition:
- every bit outside the declared fields is zero;
- a capability is present exactly when the function declares one, and it is of the declared kind.

For a buffer call, in addition:
- a memory capability is present;
- word 1 is zero;
- the request length is within the declared maximum and the buffer;
- the buffer can hold the largest reply.

The request is then **copied into the server's private memory** before it is decoded (the client could change its buffer meanwhile, MC-2.11). Decoding checks every length against its bound, booleans, enum cases, UTF-8, and that no bytes are left over.

A capability that is not accepted is dropped at once. The server answers a rejected request with `wire::reject`. A server may answer later: `Call::defer` keeps the right to reply (`IPC_SAVE_REPLY`) while it receives other requests. The kernel checks only rights and envelope limits; the schema check runs in the receiving service.

## Evolution (MC-12.4)

A change that alters the meaning or layout of an existing function increments the major version. Adding a function at the end increments the minor version: old clients never call it, and old servers answer it with 0x80. The minor and patch versions are not sent on the wire.

## Interfaces

| File | Service | Version |
|---|---|---|
| [`idl/rtc.wit`](../../idl/rtc.wit) | `rtc` (1.1 adds `date`, needed for file times; 1.2 `set`, for the client with the setting badge, 211-KRN-0051; 1.3 says `now` and `date` are UTC and adds `zone` and `set-zone`, the time zone the clocks show as a fixed offset, set by the same client, 000-APP-0099) | 1.3.0 |
| [`idl/tts.wit`](../../idl/tts.wit) | `tts` | 1.0.0 |
| [`idl/nn.wit`](../../idl/nn.wit) | `nnd`, the model service (262-APP-0112; the design is [docs/nn/runtime.md](../nn/runtime.md) 1.0): checked `nrl.model/0` packages loaded read-only, sessions bound to the caller's badge with their budgets, jobs submitted and waited for, cancelled; `retire` and `retire-all` from the shell's badge 0 only, before a badge is reused | 1.0.0 |
| [`idl/audio.wit`](../../idl/audio.wit) | `audio_gw` (`wait` is answered later, from the playback interrupt: `Call::defer`; 1.1: the microphone has one owner at a time, others get `busy`) | 1.1.0 |
| [`idl/serial.wit`](../../idl/serial.wit) | `usb_serial`, for the link service alone (800-LNK-0002, 800-LNK-0003): a USB-serial converter's line, 8N1 without flow control; `info`, `configure` (the rate), `write`, `read` and `wait` for received bytes, which the driver keeps until taken | 1.0.0 |
| [`idl/shell.wit`](../../idl/shell.wit) | the shell, for a window manager and what the shell's window starts (`REQUEST_SHELL`, lent in `SLOT_SHELL`; `wm` passes it on to `console`): `window` opens the shell's own window and answers its id; 1.1 adds `run`, a command line run on the shell's authority — the machine-changing ones after the user agrees in the shell's window — and its output; 211-APP-0044; 1.2 adds `set-clock`, a word call so that `wm`'s Settings can stop waiting while the shell asks, 000-APP-0055; 1.3: `set-clock` from the window manager the shell started sets the clock at once, without the question, 000-APP-0060; 1.4 adds `battery`, a copy of the shell's client of the battery service for a window manager whose launch had no room left for it, 211-APP-0070; 1.5 adds `wifi`, a copy of the shell's client of the Wi-Fi station for the window manager the shell started (`refused` for any other client), whose Settings start `wifi`, 550-APP-0033; 1.6 adds `boot-notice`, the safe start's notice the shell shows above its first prompt (empty after a normal boot), which `wm` puts on its status line, 000-APP-0086; 1.7 adds `net-counters`, every card's frames as the network stack counts them, the Wi-Fi station's too, for the window manager the shell started (`refused` for any other client), whose `show = net` line gives them a second, 000-APP-0096; 1.8 adds `new-window`, another window of the shell beside those open, each a session of its own, for `wm`'s Shell menu item (`busy` with 32 open), 000-APP-0097; 1.9 adds `set-zone`, the time zone every clock shows, as `date zone`, for `wm`'s Settings, and says `set-clock`'s fields are the zone's time, 000-APP-0099; 1.10 adds `keymap`, the keyboard's layout and switch for any client, and `set-keymap`, at once for the window manager the shell started and `refused` for any other, for `wm`'s Settings, 000-APP-0048 | 1.10.0 |
| [`idl/voice.wit`](../../idl/voice.wit) | the shell, for the `voice` program it starts (voice control: `next` reports what was heard and is answered with the next order — at once, or at push-to-talk: `Call::defer`) | 1.0.0 |
| [`idl/net.wit`](../../idl/net.wit) | `virtio_net` (raw Ethernet frames; `wait` is answered from the receive interrupt; 1.1 adds transmit checksum offload: `offloads`, `send-partial`; 1.2 a frame ring shared with the stack: `attach` takes a memory capability, `kick`; 1.3 a client with `BADGE_COUNTERS` only reads the card: `info`, `counters`, `offloads`, the rest `denied`) | 1.3.0 |
| [`idl/socket.wit`](../../idl/socket.wit) | `netstack` (`ping`, `resolve` and `tcp-connect` are answered when the network answers: `Call::defer`; what a client may reach comes from its badge; 2.1 adds `interfaces`, one per card; 2.2 `offload`, operator only; 2.3 lets the policy badge `resolve` the policy's host names, 351-NET-0003; 2.4 `sockets`, the open sockets: every one for the operator, its own for a grant, 000-NET-0013); 2.5: the default card's rule, the preferred card, each card's kind (550-NET-0025); 2.6 `ping-with`: an echo of a chosen size and pattern, the reply's size and data checked (000-NET-0040); 2.7: a rule of address 0 reaches any public address on its port, TCP only, the browsing grant, each new host logged (000-NET-0049) | 2.7.0 |
| [`idl/netpolicy.wit`](../../idl/netpolicy.wit) | `netpolicy` (a grant is handed over by a word call, `take`, which carries the capability; 1.1 adds `lines`, `add` and `remove`, the policy changed while the system runs, 108; 1.2 adds the error `denied`: a launcher's client, `BADGE_LAUNCH`, may only launch, 800-NET-0038) | 1.2.0 |
| [`idl/parse.wit`](../../idl/parse.wit) | `parse` (bounded bytes from outside in, typed records out; since 1.1 `channel` and `manifest`, a release channel and a boot manifest a page of 8 files at a time (351-NET-0011); since 1.2 `model`, a model of a model disk's `MANIFEST.json` with its files a page of 8 at a time (251-STO-0014); stateless, a refusal logged with the client's PID; 109-NET-0008; 1.3 `http-response`, the head and whether the connection closes, 750-NET-0019; 1.4 `json`, a JSON text's values in document order within stated bounds, 750-NET-0021; 2.0 one head, `http-response`, with the content coding, media type, `Location` and `Retry-After`, 1.x's `http-head` gone, 000-NET-0047) | 2.0.0 |
| [`idl/tls.wit`](../../idl/tls.wit) | `tls` (a client lends its flow with a word call, `attach`, that carries the capability as a parameter; the handshake runs inside `connect`; 1.1 adds `connect-pinned`, a server known by the SHA-256 of its public key, 351-NET-0002; 1.2 `connect-pins`, one of two leaf or SPKI pins with the name, validity and purpose still checked, 750-NET-0020) | 1.2.0 |
| [`idl/ssh.wit`](../../idl/ssh.wit) | `ssh` (files read over SFTP for programs, through the flow a client lends with `attach`; the server known by the SHA-256 of its ssh-ed25519 host key; logged in as the device, each login signed by `keystore`'s `ssh-login`; 351-NET-0027; started by `init` since ABI 12, its client in the shell's slot 34, 351-KRN-0115) | 1.0.0 |
| [`idl/wifi.wit`](../../idl/wifi.wit) | `wlan`, the 802.11 station (550-NET-0018): scan, join with a passphrase, status, leave, forget; one endpoint with `idl/net.wit`, told apart by the badge (`BADGE_WIFI`, `BADGE_WIFI_CHANGE`); agreed with the tools track for `wifi`; 1.1: the country, from the regulatory catalogue, asked for and applied apart, kept by `keystore`, for 550-APP-0082 (550-NET-0035; answered `unsupported` until the catalogue is in the station) | 1.1.0 |
| [`idl/wlanhw.wit`](../../idl/wlanhw.wit) | `bcm_wifi`, for the station `wlan` alone (raw 802.11 frames without their FCS: the channel, the receive filter, `send` answered with the transmit status, `receive` or a lent receive ring with the signal, channel and rate of each frame; the driver keeps nothing of 802.11 else; agreed by `NET` with `DRV`, 550-NET-0012, for 550-DRV-0027; 1.1: `bands` bit 2 names OFDM on 2.4 GHz, and without it the station is 802.11b there, 550-NET-0028; 1.2: `set-country`, a country's limits per channel applied under the radio's own, agreed with `DRV`, 550-NET-0035) | 1.2.0 |
| [`idl/effector.wit`](../../idl/effector.wit) | `effector_gw`, the Effector agent's gateway (750-EFF-0003): its clients are the session `effector` and the shell's owner client, told apart by the badge; `status`, `service-control`, transfers out (`open-log`, `open-file`, `open-screen`, `read`, `close`) and in (`deliver-begin`, `package-begin`, `write`, `deliver-commit`, `package-commit`), `update-check`, `reboot`, `test-run`, `cancel`; the owner's `reload-policy` and `audit`. Each request carries its command, initiator, reason and deadline, and is decided by the owner's policy ([docs/effector/policy.md](../effector/policy.md)) | 1.0.0 |
| [`idl/update.wit`](../../idl/update.wit) | `updater` (`check`, `fetch`, `apply`, `rollback`, `status`; releases checked with the release and boot keys built in, staged in the other slot; 351-UPD-0007; 1.1 adds `metadata`: the channel it holds and whether its expiry has passed, and `apply` refuses a staged release whose channel has expired, [351-UPD-0025](../../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done); 1.2: the Effector gateway's client, badged `BADGE_UPDATE_GATEWAY`, may `fetch`, `apply` and `rollback` only while init's `remote-update` counts, 751-KRN-0185; 1.3: that client alone hands over a release as one ZIP bundle, `install-begin`, `install-write` and `install-commit`, which the updater checks as any source, 751-UPD-0032) | 1.3.0 |
| [`idl/window.wit`](../../idl/window.wit) | `windows` (word calls `surface`, `waker` and `client` return capabilities: the program's and the manager's leases come from separate roots) | 1.0.0 |
| [`idl/tpm.wit`](../../idl/tpm.wit) | `tpm` (no raw commands: `seal` and `unseal` for the seal badge, `info` for anyone; 351-DRV-0015) | 1.0.0 |
| [`idl/keystore.wit`](../../idl/keystore.wit) | `keystore` (no call returns the private key; `sign` only for the signer's badge, a purpose and a budget; 1.1: the Wi-Fi station's remembered networks, each handshake's keys only to the station's badge, 550-NET-0024; 1.2: `ssh-login` signs only an SSH public-key login with the device key, only for the SSH service's badge and within its own budget, and `public-key`, 351-NET-0027; 1.3: the Wi-Fi station's country, two letters, only to the station's badge, 550-NET-0035; 1.4: a program's secrets, `secret-put`, `secret-get`, `secret-delete` and `secret-list`, only through the client the loader gets for that program (`secrets-client`, `secrets-name`, `secrets-adopt`, its badge alone) and lends in `SLOT_SECRETS`, one sealed file per program, the error `missing`, 000-NET-0045, 000-KRN-0182) | 1.4.0 |
| [`idl/blockstore.wit`](../../idl/blockstore.wit) | `blockstore` (blocks by CID: `put` of `raw` or `dag-cbor` (a node checked first), `get` checked against the CID, `has`, `stat`; names: `publish` by compare-and-swap of complete roots, `resolve`; rights by badge; issues 300-STO-0002, 0004, 301-STO-0002, 302-STO-0001; started at boot over `ramdisk#1`; 1.1 adds `collect`: what no name retains and no lease protects, 303-STO-0001; 1.2 adds `unpublish`, `history`, `pin`, `unpin`, `pins`, `usage` and the error `quota`: a name keeps 4 versions and can be removed, an owner pins objects within its quota, 303-STO-0002..0004; 1.3 adds `commit` (up to 8 names, all or none) and `snapshot`, 304-STO-0007; 1.4 adds `names`: the current names by prefix, 16 a page, 300-STO-0018; 2.0 says how far each write reached, `volatile`, `flushed` or `copied`, in `publish`, `unpublish`, `commit`, `pin` and `stat`, and refuses 1.x, [307-STO-0019](../../issues-done/307-STO-0019-how-far-a-write-reached.done)) | 2.0.0 |
| [`idl/block.wit`](../../idl/block.wit) | `ata`, `ahci`, `usb_storage`, `virtio_blk`, `nvme`, `ramdisk` (client: `vfs_server`; 1.1 adds `writable`, `write` with the data as sealed read-only memory, and `flush`, served to the write badge only) | 1.1.0 |
| [`idl/usb.wit`](../../idl/usb.wit) | `usb_host` (clients: the USB class drivers `usb_hid`, `usb_storage` and `video_gw`, each badged for one device class; issue 164; 1.1 adds `reports-up-to`, reports of several packets such as a MacBook trackpad's fingers, 211-DRV-0018; 1.2 adds `select`, an interface's alternate setting, and `isochronous`, the packets of its isochronous IN endpoint, 158); 1.3: `BADGE_SERIAL` claims a USB-serial converter by its vendor and product ID and may send it vendor requests, and `reports` also polls a bulk IN endpoint of up to 64-byte packets, 800-DRV-0034 | 1.3.0 |
| [`idl/gpio.wit`](../../idl/gpio.wit) | `gpio` (clients: anyone reads; the control badge changes pins; issue 207) | 1.0.0 |
| [`idl/vfs.wit`](../../idl/vfs.wit) | `vfs_server` (client: `mind::fs`): handles of roots, directories and files, the write path, `check` (2.1), `scope` (2.2: a client confined to one directory, a capability result), `format` of the RAM disk (2.3), `fill`: a file read into the caller's lent memory object in one call (2.4, 262-KRN-0162) | 2.4.0 |
| [`idl/init.wit`](../../idl/init.wit) | `init` (client: the shell's `RUN <service> &`; 1.1 adds the lifecycle requests of `svc` and `top`; 1.2 lists up to 64 services; 1.3 `reboot`, for the updater's badged client; 1.4 `boot`; 1.5 the modes; 1.6 `replace`; 1.7 the Effector gateway's client badged `BADGE_NAMED`, limited to what the owner's policy names; 1.8 `link-console`, whether data/mode.conf turned the link's console on, 800-KRN-0155; 1.9 `boot-plan`, this boot's plan as data/services.txt set it, 173-KRN-0035; 1.10 `configuration`, where the plan came from: the file, none, or ignored by a safe start asked at the loader, 173-KRN-0036; 1.11 the permissions `remote-update` and `link-remote-update` beside the modes, 751-KRN-0185) | 1.11.0 |
| [`idl/loader.wit`](../../idl/loader.wit) | `loader` (program list, start with arguments; 1.1 adds launch sessions: `begin`, `grant`, `commit`, `abort`, `inspect`; 1.2 `inspect-requests`; 1.3 `grant-memory`; 1.4 lists 128 programs; 1.5 `commit-in-front`, issue 160; 1.6 `grant-firmware`, 351-KRN-0027; 1.7 the error `unreadable`, 211-KRN-0050; 1.8 `helper`, 109-KRN-0096; 1.9 `commit-with-input`, 000-KRN-0094; 1.10 the helpers-only badge, 109-KRN-0117; 1.11 `watch` and `stop`, 800-KRN-0138) | 1.11.0 |
| [`idl/sysinfo.wit`](../../idl/sysinfo.wit) | `sysmon` (`STAT` records and load history for the monitors; 2.0 carries the fields of `STAT` version 2: largest free block, limits, kernel memory per task, PCI location, holders; 2.1 adds `holders` of an endpoint; 3.0 adds `authority` and gives who holds what — `holders`, `authority`, the derivation links in `caps` — only to the client with the authority badge; 4.0 gives `tasks`, `endpoints`, `caps` and `cpus` from a position on, since the kernel has no fixed count of them, counts a sample's tasks in 32 bits and gives its busy share for CPUs 0–15 and for every CPU as a mean and a maximum, [171-APP-0002](../../issues-done/171-APP-0002-sysinfo-pages.done), [171-APP-0007](../../issues-done/171-APP-0007-sysinfo-every-cpu-and-capability.done); 4.1 adds `pool`, the frame pool: the machine's memory the kernel gives programs, apart from the kernel arena `memory` describes, [211-APP-0057](../../issues/211-APP-0057-top-and-free-show-the-machines-memory.md)) | 4.1.0 |
| [`idl/keyboard.wit`](../../idl/keyboard.wit) | `ps2_kbd`, `virtio_input`, `usb_hid` (layout and layout switch; the shell's `keymap`) | 1.0.0 |
| [`idl/display.wit`](../../idl/display.wit) | `compositor` (the mode and a sealed copy of the screen as a capability result; the shell's `screenshot`; 1.1 `camera`, the camera mark's heartbeat, issue 158; 1.2 `remote`, the marks of the link and the Effector agent, by the caller's badge, 800-KRN-0156) | 1.2.0 |
| [`idl/video.wit`](../../idl/video.wit) | `video_gw` (cameras, one owner a stream, frames into a lent buffer with their number and time, issue 158) | 1.0.0 |
| [`idl/log.wit`](../../idl/log.wit) | `logd` (the system log; reading needs the read badge) | 1.0.0 |
