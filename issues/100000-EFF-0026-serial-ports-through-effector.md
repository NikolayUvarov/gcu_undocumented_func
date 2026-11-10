# 100000-EFF-0026 — Serial ports through Effector: section 4 handed to the Effector project, and its answer recorded

**Type:** hand-over, record · **Owner:** `EFF` · **Priority:** P2 · **Status:** in progress (2026-11-10) · **Blocked by:** the Effector project's task 108 (sst-test-deploy) · **Main task:** [100000](100000-a-phone-the-pinephone-first.md) (stages 0 and 1: the PinePhone's first test run) · **Constitution:** MC-7.1, MC-7.4, MC-7.5, MC-7.6, MC-12.1, MC-12.3, MC-12.4

**Asked by:** the kernel session, 2026-11-08, at the maintainer's request of that day (`requests-EFF.md`, its second request, now taken into this task). **The specification:** [docs/remote-serial.md](../docs/remote-serial.md) ([Russian](../docs/remote-serial_RU.md)), 0.2.

## Problem

- **What the maintainer asked:** a remote-control module, so that a console is relayed through Effector over RS-232 and RS-485, and the Effector agent on a Linux or Windows host reaches MIND Core through MIND Link.
- **Who builds it:** the Effector project, which writes the server's agents. EFF explains what to build there and hands it over.
- **Why now:** the PinePhone's only line out before its Wi-Fi works is UART0, 3.3 V, 115200 8N1.

## Plan

1. **Hand section 4 over:** done on 2026-11-10 as sst-test-deploy `issues/108-serial-ports-console-and-mind-link.task.md` (f5eae2c). It carries:
   - the ports and the host owner's policy: console writing, DTR, RTS and break off by default; in link mode only the reading kinds;
   - the line, with RS-485's direction;
   - the console session: a ring with offsets, the capture, writes written once, the lease, the stream, the audit;
   - the link mode, through `effector485 daemon --supervised` and its API 1.1 (800-LNK-0039);
   - LNK's addition: break, DTR, RTS and line changes refused on a link line;
   - safety, platforms, tests;
   - the stages S1 to S3.
2. **The specification's 0.2** records that addition and the hand-over (this commit).
3. **Bring back the Effector project's answer** on `internal-dev-messages`:
   - the names chosen;
   - how streams travel between the server and the agent;
   - the first agent revision to try;
   - the host owner's setup steps.
4. **docs/effector.md** gets a serial-ports section once the server and an agent serve them, naming the hosts with a cable and what is on it.
5. **Later, not for the phone (stage S4):** the same console contract in MIND Core's own `effector`, through `usb_serial`. That is a task of its own when it is asked for.

## Acceptance criteria

- [x] Section 4 is a task of the Effector project's own.
- [ ] Its answer (the four points of step 3) recorded here, and passed to LNK and the kernel session.
- [ ] Stage S1 seen through hesere on a named host, adapter and machine (MC-12.1): a remote session reads the PinePhone's boot and types at U-Boot.
- [ ] docs/effector.md's serial-ports section.

## Related

[docs/remote-serial.md](../docs/remote-serial.md), [800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md), [100000-PRT-0010](100000-PRT-0010-a-16550-uart-in-memory.md), [100000-KRN-0222](100000-KRN-0222-the-kernels-console-as-a-byte-channel.md), [750](750-effector-agent.md).
