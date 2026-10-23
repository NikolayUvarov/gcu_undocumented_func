# 000-NET-0044 — The wired card without a lease after its ring's attach went unanswered (seen once)

**Type:** `netstack`, `virtio_net` (reliability) · **Owner:** network track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.1, MC-12.2

## Problem

**Seen once**, on 2026-10-23, in the `net` suite on x86. The setup: this session's container (QEMU TCG, 4 CPUs), the tree `claude/relaxed-meitner-5bmhpz` a0526865 (`main` 9b4f0672 with 351-UPD-0025 and 550-NET-0043, which touch neither driver).

- **Where.** The check of a modern-only and a legacy-only VirtIO card (`net_suite`, after the main VM). One of the two cards got no DHCP lease within the wait. The suite did not say which, and it saves no log of that VM.
- **What `netstack` logged**, and nothing after it for the whole wait:
  ```
  [NETSTACK] CARD 0 READY (FRAME RING LENT, NO ANSWER YET), WAITING FOR DHCP
  [NETSTACK] CARD 0: DRIVER WITHOUT OUR RING, ATTACHING AGAIN
  ```
- **What these lines say:**
  - the first `attach` had no answer before the stack's deadline;
  - the next `kick` was answered with an error, so the driver held no ring;
  - the stack lent a new ring.
- **What is not known.** After the second `attach` nothing was logged. Two readings fit:
  - **It timed out.** Then each `kick` afterwards also timed out: the driver stopped answering, and `kick` is silent on a timeout (`netstack/src/main.rs`, "the driver is restarting").
  - **It was answered,** and the lease still did not come.
- **It did not repeat.** The same suite on the same tree passed in full on the next run, and earlier the same day on `main` e084af99, twice.
- This is not proof of a flake and not a root cause (MC-12.2).

## Plan

- **The suite keeps that VM's log** (`mind-core-net-cards-<mode>-<cpus>cpu.log`), and names the card's mode in the failure.
- **`netstack` logs the outcome of every `attach`** (answered, refused, no answer) and a `kick` that timed out, at most once per card until the next answer. A silent stall then shows which reading holds.
- **`virtio_net`** logs once if it answers a `kick` or an `attach` late, or holds a call longer than the stack's deadline.
- **Reproduce under load:** run the check alone 20 times, and with the host loaded, on x86. Then fix the cause in the driver or the stack.

## Acceptance criteria

- The cause is found and fixed, or the logs show it to be outside both services (QEMU, the host), with that evidence.
- The check passes 20 times in a row under the load that reproduced it (MC-12.1: for that configuration).

## Related

[000-NET-0036](000-NET-0036-an-unanswered-frame-ring-stays-lent.md) (a ring kept lent until the driver says whether it holds it), 000-KRN-0129, [550-NET-0043](550-NET-0043-fifteen-tries-for-a-sent-frame.md) (the run in which it was seen).
