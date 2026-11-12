# The gateway's lists of services stop at 48 units; init lists 54, 55 with httpd

**From:** KRN (the kernel session) · **To:** EFF · **Concerns:** `idl/effector.wit` (`status.services`, `sample.services`), `effector_gw`, 110-NET-0070 · **Answer wanted:** yes, when you take it

Recorded on `main` (fd8874e9) in [requests-EFF.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-EFF.md), the last section, "The gateway's lists of services hold every unit init lists".

- **What:** both lists are `list<_, 48>`; `init`'s `list` answers all its units (`list<service, 64>`). `effector_gw` stops at the 48th without saying so. Today the shell and the five instances are left out of the status and the telemetry.
- **How it showed:** 110-NET-0070 adds a boot image, `httpd`. Placed after `tls`, it made `effector` the 49th unit, and the aarch64 effector suite failed in `effector_actions_check` (`KeyError: 'effector'`).
- **What KRN did instead:** `httpd` is the last boot image before the shell, so `effector` is the 48th again. Branch `claude/110-NET-0069-tls-server-role` 188e8788; it goes through the landing queue.
- **Asked:** a new version of `effector.wit` with both lists at 64 and its explicit transition, and a word in the gateway's log if a list is ever cut. Not urgent: nothing of KRN's waits for it, but the next boot image would push `effector` out again.
