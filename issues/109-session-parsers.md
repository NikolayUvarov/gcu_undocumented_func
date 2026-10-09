# 109 — Session parsers with minimal authority (Airlock)

**Type:** main task (network) · **Owner:** `NET` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track D ("session parsers with minimal authority") · **Constitution:** MC-11.3, MC-11.4, MC-11.5, MC-11.11, Appendix B.6

Asked for by the maintainer (2026-10-09), after 351-NET-0005 and 108. Broken down and carried out by the storage session working the network track.

## Problem

B.6 maps the network path into stages, each with its own power:

| Stage | Power |
|---|---|
| NIC driver | the device |
| Network stack | flows |
| Policy broker | grants |
| TLS service | a flow and a key-operation handle |
| Session parser | bounded bytes in, typed messages out |
| Application gateway | the endpoints it needs |

MIND Core has a process for every stage but the session parser. The response head of an HTTP download is parsed by `mind::http` inside `download`, which also holds a flow grant and a writable file client. The updater (351-UPD-0007) would parse the release channel and manifests while holding the update zone, TLS and a grant.

MC-11.11 forbids a domain that parses untrusted external input from also holding broad storage access, long-term keys or critical devices. It also asks for an authority map of each adapter's stages, which does not exist yet.

## Plan: the tasks

1. **[109-NET-0007](../issues-done/109-NET-0007-airlock-authority-map.done), the authority map** (done) (`docs/network/airlock.md`). Every adapter of external input in the system today: what it parses, in which process, with which authorities, and where that falls short of MC-11.11 and B.6. This covers:
   - DHCP, ARP, DNS and TCP in `netstack`;
   - TLS records and X.509 in `tls`;
   - the HTTP head in `download`;
   - FAT in `vfs_server`;
   - ELF in the kernel and the loader;
   - the boot manifest in the bootloader;
   - images and media in the tools.
2. **[109-NET-0008](109-NET-0008-parser-service.md), a parser service `parse`.**
   - `init` starts it with nothing but its own endpoint and the system log (109-KRN-0042): no files, no network, no spawn, no devices.
   - It turns bounded bytes into typed messages (`idl/parse.wit`), statelessly: the HTTP response head first.
   - Its parsing code is the one `mind::http` had, tested on the host.
   - If it crashes, `init` restarts it (Article 6).
3. **[109-NET-0009](109-NET-0009-download-through-the-parser.md), `download` through the parser.**
   - `download` no longer parses raw response heads. It hands the head bytes to `parse` and gets a typed head back.
   - It still checks the domain's own conditions itself (MC-11.5): the range starts where it asked, and the length fits.
   - A malformed head is refused by `parse`. A head that crashes `parse` costs a restart of `parse`, not `download`'s grant or file.
4. **Recorded, not carried out here:**
   - *Per-session parser instances:* a fresh process per session needs a spawn with no standard clients, a kernel and loader change.
   - *The external medium path of B.6:* removable volumes parsed through a read-only block capability, quarantine and authorized import.
   - *The updater's metadata* (channel, manifests) through `parse`: this goes with 351-UPD-0007.

## Acceptance criteria

- The authority map exists and the profile cites it.
- `download` passes its suite with the HTTP head parsed in `parse`, a process that holds no file, network, spawn or device authority, which `stat caps` shows.
- A malformed head is refused there.

## Related

[351-NET-0001](../issues-done/351-NET-0001-http-downloads.done), [351-UPD-0007](351-UPD-0007-updater-service.md), [docs/network/downloads.md](../docs/network/downloads.md).
