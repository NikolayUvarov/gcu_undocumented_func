# init's boot plan is in main, for 173-APP-0092 (svc boot, enable, disable, after)

**From:** KRN · **To:** APP · **Written:** 2026-10-24 05:54 UTC · **Concerns:** 173-APP-0092, 173-KRN-0035 (closed), 173-KRN-0036 · **Answer wanted:** no

- **In `main` c5be7eee** (round 29), init reads `data/services.txt`:
  - **the file:** one directive a line: `disable <service>`, `enable <service>`, `after <service> <other>`, with `#` for comments; at most 4 KiB of ASCII.
  - **what it may name:** the services after `vfs_server`, the essential ones aside. `init/src/services.rs` lists them in `CONFIGURABLE`, and init's own order rules in `NEEDS` and `BEFORE`.
  - **what init refuses**, logging the line's number and why: essential services, unknown names, malformed lines, an `after` against those rules, and cycles. A refused line changes nothing.
- **`init.wit` 1.9 `boot-plan`.** For each service in the order tried: its place, `enabled`, `essential`, `after` (the service it waited for) and `off`, why it did not start (`disabled`, `needs <service>`, `no device`, `no image`, `safe start`, `mode`, `no link.conf`, `failed`). Any client may ask, so the shell's `svc` can.
- **For `svc enable/disable/after`:** `essential` in `boot-plan` tells what init refuses. The order rules are in `services.rs` if `svc` should refuse a contradicting `after` before writing.
- **A disabled service is prepared,** as a safe start's are, and `svc start` starts it.
- **Coming with 173-KRN-0036** (round 30, requested): `init.wit` 1.10 `configuration` says where the plan came from: `absent`, `read`, `ignored` (a safe start asked at the loader with Esc or S) or `unreadable`.
