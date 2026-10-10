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
  - at most one capability: `own<memory>`, `own<endpoint>` (moved, `CAP_TRANSFER_MOVE`) or `borrow<memory>`, `borrow<endpoint>` (copied, so the client can revoke it: a lease); `borrow<firmware>` is the firmware variable privilege (`CAP_KIND_FIRMWARE`, 351-KRN-0027), copied.
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
| [`idl/rtc.wit`](../../idl/rtc.wit) | `rtc` (1.1 adds `date`, needed for file times; 1.2 `set`, for the client with the setting badge, 211-KRN-0051) | 1.2.0 |
| [`idl/tts.wit`](../../idl/tts.wit) | `tts` | 1.0.0 |
| [`idl/audio.wit`](../../idl/audio.wit) | `audio_gw` (`wait` is answered later, from the playback interrupt: `Call::defer`; 1.1: the microphone has one owner at a time, others get `busy`) | 1.1.0 |
| [`idl/shell.wit`](../../idl/shell.wit) | the shell, for a window manager that asks for it (`REQUEST_SHELL`, lent in `SLOT_SHELL`): `window` opens the shell's own window and answers its id; 211-APP-0044 | 1.0.0 |
| [`idl/voice.wit`](../../idl/voice.wit) | the shell, for the `voice` program it starts (voice control: `next` reports what was heard and is answered with the next order — at once, or at push-to-talk: `Call::defer`) | 1.0.0 |
| [`idl/net.wit`](../../idl/net.wit) | `virtio_net` (raw Ethernet frames; `wait` is answered from the receive interrupt; 1.1 adds transmit checksum offload: `offloads`, `send-partial`; 1.2 a frame ring shared with the stack: `attach` takes a memory capability, `kick`) | 1.2.0 |
| [`idl/socket.wit`](../../idl/socket.wit) | `netstack` (`ping`, `resolve` and `tcp-connect` are answered when the network answers: `Call::defer`; what a client may reach comes from its badge; 2.1 adds `interfaces`, one per card; 2.2 `offload`, operator only; 2.3 lets the policy badge `resolve` the policy's host names, 351-NET-0003) | 2.3.0 |
| [`idl/netpolicy.wit`](../../idl/netpolicy.wit) | `netpolicy` (a grant is handed over by a word call, `take`, which carries the capability; 1.1 adds `lines`, `add` and `remove`, the policy changed while the system runs, 108) | 1.1.0 |
| [`idl/parse.wit`](../../idl/parse.wit) | `parse` (bounded bytes from outside in, typed records out: `http-head`; since 1.1 `channel` and `manifest`, a release channel and a boot manifest a page of 8 files at a time (351-NET-0011); since 1.2 `model`, a model of a model disk's `MANIFEST.json` with its files a page of 8 at a time (251-STO-0014); stateless, a refusal logged with the client's PID; 109-NET-0008) | 1.2.0 |
| [`idl/tls.wit`](../../idl/tls.wit) | `tls` (a client lends its flow with a word call, `attach`, that carries the capability as a parameter; the handshake runs inside `connect`; 1.1 adds `connect-pinned`, a server known by the SHA-256 of its public key, 351-NET-0002) | 1.1.0 |
| [`idl/window.wit`](../../idl/window.wit) | `windows` (word calls `surface`, `waker` and `client` return capabilities: the program's and the manager's leases come from separate roots) | 1.0.0 |
| [`idl/tpm.wit`](../../idl/tpm.wit) | `tpm` (no raw commands: `seal` and `unseal` for the seal badge, `info` for anyone; 351-DRV-0015) | 1.0.0 |
| [`idl/keystore.wit`](../../idl/keystore.wit) | `keystore` (no call returns the private key; `sign` only for the signer's badge, a purpose and a budget) | 1.0.0 |
| [`idl/blockstore.wit`](../../idl/blockstore.wit) | `blockstore` (blocks by CID: `put` of `raw` or `dag-cbor` (a node checked first), `get` checked against the CID, `has`, `stat`; names: `publish` by compare-and-swap of complete roots, `resolve`; rights by badge; issues 300-STO-0002, 0004, 301-STO-0002, 302-STO-0001; started at boot over `ramdisk#1`; 1.1 adds `collect`: what no name retains and no lease protects, 303-STO-0001; 1.2 adds `unpublish`, `history`, `pin`, `unpin`, `pins`, `usage` and the error `quota`: a name keeps 4 versions and can be removed, an owner pins objects within its quota, 303-STO-0002..0004; 1.3 adds `commit` (up to 8 names, all or none) and `snapshot`, 304-STO-0007) | 1.3.0 |
| [`idl/block.wit`](../../idl/block.wit) | `ata`, `ahci`, `usb_storage`, `virtio_blk`, `nvme`, `ramdisk` (client: `vfs_server`; 1.1 adds `writable`, `write` with the data as sealed read-only memory, and `flush`, served to the write badge only) | 1.1.0 |
| [`idl/usb.wit`](../../idl/usb.wit) | `usb_host` (clients: the USB class drivers `usb_hid`, `usb_storage` and `video_gw`, each badged for one device class; issue 164; 1.1 adds `reports-up-to`, reports of several packets such as a MacBook trackpad's fingers, 211-DRV-0018; 1.2 adds `select`, an interface's alternate setting, and `isochronous`, the packets of its isochronous IN endpoint, 158) | 1.2.0 |
| [`idl/gpio.wit`](../../idl/gpio.wit) | `gpio` (clients: anyone reads; the control badge changes pins; issue 207) | 1.0.0 |
| [`idl/vfs.wit`](../../idl/vfs.wit) | `vfs_server` (client: `mind::fs`): handles of roots, directories and files, the write path, `check` (2.1), `scope` (2.2: a client confined to one directory, a capability result), `format` of the RAM disk (2.3) | 2.3.0 |
| [`idl/init.wit`](../../idl/init.wit) | `init` (client: the shell's `RUN <service> &`; 1.1 adds the lifecycle requests of `svc` and `top`; 1.2 lists up to 64 services; 1.3 `reboot`, for the updater's badged client) | 1.3.0 |
| [`idl/loader.wit`](../../idl/loader.wit) | `loader` (program list, start with arguments; 1.1 adds launch sessions: `begin`, `grant`, `commit`, `abort`, `inspect`; 1.2 `inspect-requests`; 1.3 `grant-memory`; 1.4 lists 128 programs; 1.5 `commit-in-front`, issue 160; 1.6 `grant-firmware`, 351-KRN-0027; 1.7 the error `unreadable`, 211-KRN-0050) | 1.7.0 |
| [`idl/sysinfo.wit`](../../idl/sysinfo.wit) | `sysmon` (`STAT` records and load history for the monitors; 2.0 carries the fields of `STAT` version 2: largest free block, limits, kernel memory per task, PCI location, holders; 2.1 adds `holders` of an endpoint; 3.0 adds `authority` and gives who holds what — `holders`, `authority`, the derivation links in `caps` — only to the client with the authority badge; 4.0 gives `tasks`, `endpoints`, `caps` and `cpus` from a position on, since the kernel has no fixed count of them, counts a sample's tasks in 32 bits and gives its busy share for CPUs 0–15 and for every CPU as a mean and a maximum, [171-APP-0002](../../issues-done/171-APP-0002-sysinfo-pages.done), [171-APP-0007](../../issues-done/171-APP-0007-sysinfo-every-cpu-and-capability.done)) | 4.0.0 |
| [`idl/keyboard.wit`](../../idl/keyboard.wit) | `ps2_kbd`, `virtio_input`, `usb_hid` (layout and layout switch; the shell's `keymap`) | 1.0.0 |
| [`idl/display.wit`](../../idl/display.wit) | `compositor` (the mode and a sealed copy of the screen as a capability result; the shell's `screenshot`; 1.1 `camera`, the camera mark's heartbeat, issue 158) | 1.1.0 |
| [`idl/video.wit`](../../idl/video.wit) | `video_gw` (cameras, one owner a stream, frames into a lent buffer with their number and time, issue 158) | 1.0.0 |
| [`idl/log.wit`](../../idl/log.wit) | `logd` (the system log; reading needs the read badge) | 1.0.0 |
