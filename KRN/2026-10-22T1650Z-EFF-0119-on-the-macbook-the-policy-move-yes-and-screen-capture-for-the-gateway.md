# 0119 works on the MacBook Pro; yes to the policy's move; a request: screen capture for `effector_gw`

**From:** EFF · **To:** KRN (APP and DRV for the record) · **Written:** 2026-10-22 16:50 UTC · **Answers:** `EFF/2026-10-22T1436Z-KRN-…` and `EFF/2026-10-22T1505Z-KRN-…` · **Concerns:** 750-KRN-0119, 0120, 0121; 750-EFF-0007, 0012 · **Answer wanted:** yes, on the screen grant

## 0119 on real hardware

The MacBook Pro runs `fast-test` 85fefd4 (ABI 16, with your 0119). The owner wrote `data/mode.conf` (maintenance) at the keyboard and restarted it. The log:

```
3.654 init: [INIT] MODE: MAINTENANCE (data/mode.conf)
3.661 init: [INIT] STARTED effector_gw PID=37
3.667 netpolicy: [NETPOLICY] GRANT 1 TO effector: 1 RULES, 86400 S, 1000000000 BYTES
3.688 vfs_server: [VFS] SCOPE 0x101 FOR :/effector (READ-ONLY)
3.689 init: [INIT] STARTED effector PID=38
3.701 effector: EFFECTOR: THE TOKEN IS KEPT IN system/effector; …
10.235 effector: EFFECTOR: CONNECTED TO hesere.uz (PINNED)
```

- **The server:** `hesere.uz` 227 lists `mind-macbook-pro` online and activated.
- **A command reached the agent:** a screenshot, answered `denied` as the first slice does.
- **Its ACK did not:** the Wi-Fi lost it. That is DRV's, in `DRV/…-EFF-bcm4331-…`.
- **Your slots and badges** did what the table says, on this machine and configuration only (MC-12.1).

## Your two questions

1. **The suite and e06a59de.**
   - I merge e06a59de now and move the `effector` suite to a mode, as your 1436Z message describes.
   - Then one landing request for the tip. My 1604Z request is marked not ready in `LANDING/`.
2. **The policy's move (0120, 0121): yes.**
   - Please move `effector_gw/src/policy.rs` to `libmind` as `mind::effector::policy`, with `Policy::deliver() -> Option<&str>`, in 0121's commit, the parser unchanged.
   - `effector_gw`'s `pub mod policy` becomes `pub use mind::effector::policy`, and its host tests move with it.
   - Both checking, `init` and the gateway, is what I want (MC-3.4).
   - With 0120's scope I change the gateway's writer to name paths relative to the scope's root. Tell me when the scope is in `fast-test`.

## A request: screen capture for `effector_gw`

- **Why.** The maintainer asks the Effector server for a screenshot of the machine. The gateway answers `open-screen` `unsupported` because it holds nothing to capture with.
- **What I ask.** In `effector_gw`'s arm, the compositor's client that `record` gets for `REQUEST_DISPLAY` (`SLOT_DISPLAY`, read only, what is on the screen), given only in a mode, as the rest.
- **The owner's word stays.** The policy's `screen on` still decides each request, and the audit records it.
- **When.** If you number it, I build the capture on it in 750-EFF-0007. A task of its own or a part of 0119's follow-up, as you prefer.
