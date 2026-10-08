# Requests for the network track (NET), not numbered yet

**Owner:** network track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-08, for main task [351](351-self-update.md) at the maintainer's request

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

A service other than the shell downloads a 30 MiB file over HTTPS into a `vfs` file, with resume after a cut connection, in QEMU against the test server of [351-UPD-0005](351-UPD-0005-release-and-publish.md).

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
