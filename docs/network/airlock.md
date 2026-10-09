# Airlock: the authority map of the adapters

**Version:** 0.1 (2026-10-09) · **Track:** `NET`, task [109-NET-0007](../../issues-done/109-NET-0007-airlock-authority-map.done) of main task [109](../../issues/109-session-parsers.md) · **Constitution:** MC-11.3, 11.4, 11.5, 11.6, 11.9, 11.11, Appendix B.6

MC-11.11 asks that every adapter of external input have an authority map of its stages: transport, cryptographic operations, parsing, policy and application effect. It also asks that a domain parsing untrusted input hold none of the following:
- universal granting of authorities;
- broad storage access;
- long-term keys;
- direct control of critical devices.

This page is that map for MIND Core as built. Each row says what a stage parses, what the process that parses it holds, and whether that meets the rule. Where it does not, it names the task that closes the gap or says that none does yet.

The authorities come from [docs/profile/bootstrap.md](../profile/bootstrap.md), which lists what `init` gives each service, and from the launchers' grants to programs.

## The network adapter (B.6's network path)

| Stage (B.6) | Process | Parses | Holds | Against MC-11.11 |
|---|---|---|---|---|
| NIC driver | `virtio_net` (one per card) | the virtqueues the card writes | the card's memory BAR, an MSI-X vector, its DMA region, its endpoint | meets B.6: the device and nothing else |
| Network stack | `netstack` | Ethernet, ARP, IPv4, ICMP, UDP and TCP; DHCP offers and DNS answers (smoltcp) | clients of the card drivers, its endpoint | meets it: flows only, no device, no files, no spawn |
| Policy broker | `netpolicy` | the policy file: local, changed only after the user's yes ([108](../../issues-done/108-editable-network-policy.done)); names reach it already resolved, as addresses from the stack | a stack client to mint grants from, the stack's policy client, a VFS client whose badge opens only its own private directory for writing | parses no external input |
| TLS service | `tls` | TLS 1.3 records and handshake messages, X.509 chains (rustls, rustls-webpki) | an `rtc` client, a read-only VFS client (the root store), the key service's signer client; no network of its own: clients lend their flows | meets the rule for keys (MC-11.9): the signer client signs only data of the form of a TLS 1.3 CertificateVerify, at most 4096 times a boot. Gap: one process serves every client's sessions |
| Session parser | — | — | — | **missing:** the HTTP response head is parsed by the application ([109-NET-0008](../../issues/109-NET-0008-parser-service.md), [0009](../../issues/109-NET-0009-download-through-the-parser.md)) |
| Application gateway | `download` | today, the HTTP response head (`mind::http`) | its own flow grant (only what the policy names), a VFS client confined to the file's directory, writable in `data/` and `ram:`, its console | **gap:** it parses untrusted input while holding a writable file client; closed by 109-NET-0009 |
| The shell's network commands | `shell` (`fetch`, `https`, `nslookup`, `ping`) | nothing beyond splitting lines: it prints only printable ASCII and line ends of a response, so a server cannot send escape sequences into the console | everything the shell holds, among it the operator's stack client (every destination) | parses no content. Its unrestricted operator client is recorded in the profile's MC-11.6 row |

## Other adapters of external input

| Input | Process | Holds | Against MC-11.11 and B.6 |
|---|---|---|---|
| FAT volumes: the boot disk, USB sticks, the model disk | `vfs_server` | write-badged clients of every running block driver and the RAM disk | **gap:** B.6's external medium path parses a removable medium through a read-only block capability in an isolated volume parser, then quarantine and authorized import. Here one process parses every volume and can write to every disk. No task yet: recorded under 109 |
| USB descriptors and reports | `usb_host`, `usb_storage`, `usb_hid` | `usb_host`: the xHCI controller's registers and DMA; the others: badged clients of it, `usb_hid` also the input privilege | a DMA-capable driver without an IOMMU is out of scope (threat model): it defeats every memory guarantee |
| ELF images of applications | the kernel (`SPAWN`), after `loader` reads the file | the kernel is the TCB | **gap:** an ELF parser runs in ring 0. Boot images are checked against the signed manifest before they are parsed (350); applications are not checked yet (350-UPD-0004) |
| The boot manifest | the UEFI bootloader | everything, before the kernel | only bytes whose Ed25519 signature checked are parsed (350-UPD-0003) |
| Images, video, audio files (BMP, JPEG, AVI, WAV) | the programs that show or play them (`view`, `camera`, `listen`, `hear`) | what their launcher lent them: a read-only or confined file client, a window or the screen | the parser holds only the program's own authorities; no keys, no devices |
| Release metadata: the channel and the manifests | the updater (351-UPD-0007, not built) | the update zone, TLS, a flow grant (planned) | to be parsed in the parser service when the updater is built (109) |

## Keeping the map

Every new adapter, or new authority for a process in these tables, changes this page in the same commit (AGENTS.md section 3, step 5). The profile's MC-11.3/11.11 row cites it.
