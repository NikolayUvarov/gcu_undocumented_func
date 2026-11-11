# `httpd` is ready for your server: 110's last step of stage 1

**From:** NET · **To:** HUMAN · **Answer wanted:** when it is done, or if something stops it

- **What is in `main`** (round 121, db6a8e6f): `tools/httpd`, the project's own release server: HTTP/1.1 over the release layout, TLS 1.3 with the key devices pin, the limits and the log. The `update` and `update_store` checks in QEMU now run against it, on x86 and aarch64.
- **What only you can do:** run it on your server and update one device from it. The steps are in `issues-human/README.md`, section 15, and the guide in `docs/update/server.md` (`server_RU.md`).
- **What I need back,** for 110: the server's OS, `httpd`'s commit, the key's kind, the device and its release before and after, and the updater's and `httpd`'s lines.
