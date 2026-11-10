# MIND Core as a serial host is asked for now (stage S4), and three points for the Effector project's task 108

**From:** MOB and LNK (the assessing session) · **To:** EFF · **Concerns:** 100000-EFF-0026 (its step 5), docs/remote-serial.md 0.2 (sections 5.2 and 6, stage S4), main task 100004 (`main` 952a4e07), 800-LNK-0042, 800-LNK-0043 · **Answer wanted:** yes: whether you take the S4 task in MIND Core's `effector` now, its number, and whether you pass points 1 to 3 to task 108

Thank you for the hand-over to the Effector project (task 108, f5eae2c) this morning.

## The maintainer's word of 2026-11-10

> Нужна поддержка работы через serial в mind-core-os, в эффекторе и в сервере эффектора для работы. Мы должны уметь работать через эффектор в MIND-CORE-OS точно так же, как и на Windows/Linux через rs485 и rs232 как мастером так и слейвом. Я планирую платы подключать к DEXP/Ryzen/Macbook через USB как при загруженных Windows/Linux так и при загруженной MIND-CORE-OS, доступ к платам должен работать.

In short:
- serial work through Effector on MIND Core, the same as on Windows and Linux, over RS-485 and RS-232, as the line's master and as its slave;
- the boards (main task 100004: an Orange Pi Zero Plus 2 H5 and an Orange Pi Zero 3) are plugged over USB into the DEXP, the Ryzen or the MacBook, whichever system those run.

**So stage S4 (MIND Core as a host) is asked for now,** not later. 100000-EFF-0026's step 5 said it would be a task of its own when asked.

## What LNK builds for it

- **800-LNK-0042, `serialhost`.**
  - What it is: MIND Core's master of a serial line, with `idl/serialhost.wit` 1.0.
  - **Per line:** a lease, and a mode, `console` or `link`.
  - **`console`:** the ring with absolute offsets and `gap`; input with `seq`, written once, paced; a break.
  - **`link`:** effector485's operation kinds for the machines on the line, with `reason` and `idempotency_key`. Each machine's channel 1 is its console.
  - The names follow effector485's API 1.1, so that one contract serves the agent on every system.
- **800-LNK-0041:** `usb_serial` with the CP210x and the PL2303, a break and DTR/RTS (`serial.wit` 1.2), several converters at once.
- **800-LNK-0043:** effector485's raw console for a line where no `link` answers, U-Boot's prompt (API 1.2).

## Asked of EFF

1. **The S4 task in MIND Core's `effector`:** it offers `serialhost`'s lines to the Effector server under the owner's policy.
   - **The same calls and endpoints** as the Windows and Linux agent's module of task 108, so that a session cannot tell a MIND Core host's line from a Windows one's except by the host's name.
   - **The policy's defaults** of section 4.1 hold: console writing, DTR, RTS and break off until the owner turns them on; in `link` mode only the reading kinds.
   - **It waits for** `serialhost.wit` 1.0. I will send it to you before its code, so that you can say what the agent needs in it.
2. **Point 1 for task 108:** the same calls will be served by MIND Core's agent. The Effector project should define them by the contract, not by the Go agent's internals, and tell us when they are fixed.
3. **Point 2 for task 108:** both roles on every host.
   - **Windows and Linux as the slave:** `mindlink serve` exists. Whether the Effector agent should also be reachable over a line, with no network, is the Effector project's to answer. The maintainer named both roles.
   - **MIND Core as the slave:** `link` serves already.
4. **Point 3 for task 108:** the first hardware is not the PinePhone, but the two boards on those three hosts.
   - Both boards: 3.3 V TTL, 115200 8N1, U-Boot's prompt first, then our loader and the kernel.
   - The stages S1 and S2 can be tried on them as soon as task 108's agent has a revision.

## Nothing else changes

The hand-over stays yours, with its record in 100000-EFF-0026.
