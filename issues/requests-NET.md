# Requests for the network track (NET), not numbered yet

**Owner:** network track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-08, for main tasks [351](351-self-update.md) and [550](550-network-on-real-hardware.md) at the maintainer's request

The network track numbers its own tasks (`NNN-NET-MMMM`), so requests from other tracks wait here. The network track turns each into a task and removes it from this file, and the file goes when it is empty.

## HTTPS downloads for a service (351, phase 2)

### Problem

Only the shell may use `tls` (`SLOT_TLS`; there is no `REQUEST_TLS`). The shell's `https` shows 2 KiB and saves nothing, and there is no HTTP client a program can use. The trust roots are `tlsroots.pem` on the boot disk, not shipped by the build and not authenticated. `netpolicy` matches IPv4 addresses only, and a grant defaults to 16 MiB.

### Plan (a proposal; the network track decides)

- A client for programs: an HTTP/1.1 GET with `Range` and resume, streaming into a `vfs` file through the caller's own vfs client. It goes in `libmind` or a small session service; it does not belong in the shell.
- Trust for the update server: either a root store shipped with the release and covered by its signature, or the server's key pinned in the channel configuration.
- `netpolicy`: names (resolved at grant time), and a grant volume large enough for a release (about 30 MiB today).
- The grant flag that lets `init` give `updater` TLS is a kernel task: [351-KRN-0014](351-KRN-0014-trial-boot-and-confirmation.md).

### Acceptance criteria

A service other than the shell downloads a 30 MiB file over HTTPS into a `vfs` file, with resume after a cut connection, in QEMU against the test server of [351-UPD-0005](../issues-done/351-UPD-0005-release-and-publish.done).

## An SSH client (351, phase 3)

### Problem

No SSH code exists. The maintainer wants updates fetched over SSH as well as HTTPS. Most of the cryptography exists in `tls` and `keystore`: X25519, Ed25519, ChaCha20-Poly1305, AES-GCM, SHA-2 and HMAC.

### Plan (a proposal)

- SSH 2 client transport: `curve25519-sha256` key exchange, `ssh-ed25519` host keys checked against a pinned key, `chacha20-poly1305@openssh.com`.
- Public-key login with the device's key (next request).
- The SFTP subsystem for reading files, or `exec cat` as a first step.
- The updater fetches the same release files over it. Authenticity still comes from the release signature, not from SSH.
- The licences of any crate used go into THIRD_PARTY.md.
- An SSH server on the device (for pushing) needs TCP listen in `netstack`. It is not part of this request.

### Acceptance criteria

In QEMU, against OpenSSH on the host, a service logs in with the device key, checks the pinned host key, and reads a file over SFTP into `vfs`.

## A persistent device key (351, phase 3)

### Problem

`keystore`'s Ed25519 key is made anew at every boot and kept only in memory, and it signs only TLS 1.3 CertificateVerify. A server cannot authorize a key that changes at every boot.

### Plan (a proposal)

- Keep the key across boots, sealed: encrypted with a key the device can recover and a copy of the disk alone cannot. TPM where present; otherwise state plainly what protects it.
- Add a signing purpose for the SSH login, under its own budget.
- Show the public key to the shell, so it can be put on the server.

### Acceptance criteria

The device key is the same after a reboot, its public key is shown, and it signs only the purposes listed.

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
