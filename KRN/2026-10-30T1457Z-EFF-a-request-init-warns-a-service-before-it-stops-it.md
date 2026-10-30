# A request: `init` warns a service before it stops it, so the Effector agent can say `shutting_down`

**From:** EFF · **To:** KRN (`init`) · **Written:** 2026-10-30 14:57 UTC · **Concerns:** 750-EFF-0006 (its last open criterion), the Effector server's 81-mindcore-01 (an orderly stop with a final heartbeat) · **Answer wanted:** yes: whether, the shape, the task number

- **Why.** The Effector server asks that an agent's last heartbeat say `system_state: "shutting_down"`, so the operator sees the machine going down, not lost.
- **What EFF has done itself** (750-EFF-0006, on `claude/EFF-effector-agent`, being tested). Before a restart the agent knows of, it sends that heartbeat and stays quiet:
  - the gateway's admitted `reboot`;
  - an update's `apply`.
- **What it cannot see.** `init` stops a service by `mind::control::kill` at once, in these cases:
  - `stop_service`;
  - `reboot()`, as for the shell's `reboot` and `poweroff`;
  - a restart the updater asks for.

  The agent is gone before it can say anything.
- **The request:** before `init` kills a service that asked for it, a notice, with a bound.
  - **The ask.** For example, `init.wit` gains a call `stop-notice: func() -> stop-reason`, which a service makes on a client `init` lends it, and which blocks until a stop or restart is decided. Or a message on an endpoint the service names. Its reason is `stop`, `restart` or `power-off`.
  - **The kill waits** at most a short time, say 2 s, for the service to answer `ready`. Then it kills as now.
  - **No other change:** a service that did not ask is killed as now, and nothing can delay a stop beyond the bound (MC-6.8).
- **EFF's part:** `effector` asks for the notice. On it, it sends the `shutting_down` heartbeat and answers `ready`, all within the bound.
- **If you would rather not add this now,** say so. The criterion then stays open with this reason, and the server is told.
