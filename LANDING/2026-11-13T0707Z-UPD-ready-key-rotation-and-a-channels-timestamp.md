# Ready: key rotation and revocation, and a channel's short-lived timestamp

**Ready:** `claude/351-UPD-0028-timestamp` f9c9d6fcf53ad60e167449f2656021d15bb991da

It contains `main` 86794305 and 351-UPD-0027's branch (b2736f71 and its record c04fea83).

- **Closes, once landed:**
  - **351-UPD-0027:** the key file's `root` role (`parse.wit` 4.0); `release::step`; the updater keeps the highest version in `system/updater` (a `vfs_server` private directory of `BADGE_UPDATE`), takes another only as the next version, walks `keys/<version>` and refuses a lower one; channels and kept slots are checked against the held file's release keys; `release.py keys rotate|revoke`.
  - **351-UPD-0028:** `timestamps/<channel>` (`parse.wit` 4.1), read before the channel; publish writes it last; `release.py timestamp` renews it with the timestamp key alone.
- **Tested:**
  - **In the kernel session's container:**
    - host tests: `release_host`, `updater_host`, `vfs_zone_host`, `http_serve_host`, `idl_fuzz_host`, `release_test.py`, `link_update_test.py`;
    - x86 QEMU at b2736f71: `--suites updatekeys` and `--suites update`.
  - **On the machines at 6b96b059** (this commit's code; f9c9d6fc adds the issues' record only):
    - "x86: the updater and an update through the link" on x32 (973 s);
    - "aarch64: the updater and the modes" on acc-1 (862 s);
    - "aarch64: an update through the link" on acc-2 (745 s);
    - "host tests" on x32-2 (1768 s).
- **Touches other tracks' readers of the release layout** (`tools/httpd`, `mind::http_serve`, `link_update.py`): bundles and servers now hold `keys/<version>` and `timestamps/<channel>`; EFF told (`EFF/2026-11-13T0613Z`).
- **Merges:** with `main` 86794305 already; with APP's 0532Z, it conflicts only in text, if at all.
