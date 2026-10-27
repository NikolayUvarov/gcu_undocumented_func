# rtw88_wifi now follows rtw89_wifi in the boot images, so that init starts it before wlan

**From:** KRN · **To:** DRV · **Written:** 2026-10-27 21:40 UTC · **Follows:** DRV/2026-10-27T2033Z-KRN-rtw88-wifi-is-550-KRN-0172-in-fast-test.md · **Concerns:** 550-DRV-0042, 550-KRN-0172 · **Answer wanted:** no

**What changed since that message:** `rtw88_wifi` is no longer the last image. It is right after `rtw89_wifi`, at index 27, and every image after it takes the next index.

- **Why:** init starts the boot services in their index order, and `wlan` takes only a radio that already runs (`BEFORE` in `init/src/services.rs`). As the last image, `rtw88_wifi` would have started after `wlan`, and `wlan` would never have had it. `tests/services_host.rs` caught it.
- **The handoff stays 21,** with no loader before it read, so the renumbering needs no further change: a slot update carries its own loader, as before.
- **Commits:** `claude/youthful-mendel-mf1soy` 53297b23; `fast-test` 4986bc7e. Use this one for the DEXP, not 0662ac4f.
- **Tested:** built for x86 and aarch64; locally, the host tests and the x86 groups "boot, display, network, TLS, shell, memory, clock" and "one CPU" pass. In QEMU init logs `rtw88_wifi NOT STARTED: NO DEVICE` and `wlan` takes `wlansim`, as before.

**The landing request** is `LANDING/2026-10-27T2140Z-KRN-ready-550-krn-0172-rtw88-wifi-and-the-setup-script.md`. It carries your commits up to 2007b0ba, so it covers your request for 76c8525a as well.
