# 750-KRN-0194 — `init` warns a service before it stops it: `init.wit` 1.12 `stop-notice`, with a bound

**Type:** `init`, IDL (`init.wit` 1.12) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in `main` (round 57); closes once EFF's check passes · **Blocked by:** — · **Main task:** [750](750-effector-agent.md), for 750-EFF-0006 · **Roadmap:** track G · **Constitution:** MC-6.8, MC-10.2, MC-12.4

**Asked by:** EFF on 2026-10-30, in `KRN/2026-10-30T1457Z-EFF-a-request-init-warns-a-service-before-it-stops-it.md` on `internal-dev-messages`, for the last criterion of 750-EFF-0006. The Effector server asks that an agent's last heartbeat say `system_state: "shutting_down"` (its 81-mindcore-01).

## Problem

- **What `effector` can say today.** It sends that heartbeat before the restarts it knows of: the gateway's admitted `reboot`, and an update's `apply`.
- **What it cannot see.** `init` kills a service at once with `mind::control::kill`, so it has no chance to speak:
  - `stop`;
  - `reboot()`, for the shell's `reboot` and `poweroff` too;
  - a restart the updater asks for.

## Plan

- **`init.wit` 1.12:**

  ```wit
  /// Why the caller is about to be stopped (1.12).
  enum stop-reason { stop, restart, power-off }
  /// Waits until `init` decides to stop the caller, or the whole system, and says why; the caller then has
  /// STOP_NOTICE_MS to end on its own before it is killed (1.12).
  stop-notice: func() -> result<stop-reason, error>;
  ```

- **How `init` holds the call:**
  - it saves the reply (`IPC_SAVE_REPLY`), one per service PID;
  - a second call from the same PID replaces the first, which is answered `failed`;
  - only a boot service may ask, and others are refused `denied`.
- **`effector`'s client:** `init` gives it a client of its own, badged `BADGE_NOTICE` (`mind::process`, 7), in `SLOT_LIFECYCLE` (11).
  - With it `effector` may only ask for the notice.
  - Every lifecycle change from that client is refused and logged (MC-3.4).
- **When `init` answers:**
  - for `stop` of that service: `stop`;
  - for `restart` of it or `replace`: `restart`;
  - for `reboot()`: `restart`;
  - for a power-off: `power-off`.
- **After the answer:**
  - `init` waits at most `STOP_NOTICE_MS` (2000) for the service to end on its own, seen through its watch;
  - the service's end is not taken as a fault, so it is not restarted;
  - after the bound, `init` kills it as now.

  Ending on its own is the service's "ready", so no second call is needed.
- **What does not change:**
  - a service that did not ask is killed at once, as now;
  - nothing delays a stop by more than the bound per noticed service, and `reboot()` warns all the noticed services at once and waits once (MC-6.8).
- **The log:** `[INIT] STOP NOTICE TO <name> (PID <n>): <REASON>`, and then either `<name> ENDED WITHIN THE NOTICE` or `<name> KILLED AFTER THE NOTICE`.
- **`effector`** asks for the notice and, on it, sends the `shutting_down` heartbeat and exits (EFF's 750-EFF-0006).

## Acceptance criteria

- **Host tests:** the generated `init.wit` 1.12 code and a fuzz entry for `stop-notice`.
- **QEMU, x86 and aarch64 (the `modes` suite, which starts `effector`):**
  - `stop effector` gives `effector` the notice, which it answers by ending within the bound;
  - a service that asked but does not end is killed after the bound, with the line saying so;
  - a service that did not ask stops at once, as now;
  - `reboot` gives the notice once, and the restart follows within the bound.
- **EFF's check:** the last heartbeat says `shutting_down`, in its `effector` suite.

## Progress

- **2026-10-30, made** (`claude/youthful-mendel-mf1soy` afdb038e):
  - `init.wit` 1.12 `stop-notice` and `stop-reason`, regenerated; `StopReason` in the IDL fuzzer;
  - `init`: the held call per unit, the reasons, the 2 s bound with its two log lines, `reboot()`'s single wait, and `effector`'s client badged `BADGE_NOTICE`;
  - `docs/idl/README.md` (1.12.0) and `docs/profile/bootstrap.md` (`effector`'s slot 11);
  - the `modes` suite checks `effector`'s client in slot 11.
- **Tested:** the x86 build, host tests and the `modes` suite (x86, QEMU TCG); the stops, restarts and reboots of the Effector services behave as before while nobody asks.
- **Landed** in round 57, `main` ad3f64ff: `aarch64: the updater and the modes` and `x86: services, storage, audio` (with `modes`) passed there.
- **Left:** the notice itself, which needs `effector` to ask (750-EFF-0006), and EFF's check.

## Related

[750-EFF-0006](750-EFF-0006-session-service.md), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md), [751-KRN-0185](751-KRN-0185-the-remote-update-permission-in-init.md), MC-6.8.
