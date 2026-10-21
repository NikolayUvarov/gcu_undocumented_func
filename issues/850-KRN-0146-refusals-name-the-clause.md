# 850-KRN-0146 — A refusal names the clause it enforces

**Type:** `init`, `loader`, kernel log lines · **Owner:** kernel session · **Priority:** P3 · **Status:** made (2026-10-21); the landing left · **Blocked by:** — · **Main task:** [850](850-the-cultural-code.md) (the kernel track's part) · **Constitution:** MC-10.2, MC-12.3

## Problem

The system's voice in its logs is precise but anonymous. For example:
- `init` refuses a mode change or a reboot to a client without the badge;
- the loader refuses a program;
- the kernel refuses a capability operation.

None of these says which requirement it enforces. A reader of a log, or of a screenshot, cannot connect the refusal to the Constitution, though every requirement has a stable identifier.

## Plan

- **The line.** A refusal that enforces a constitutional requirement ends with its clause, for example `[INIT] MODE CHANGE REFUSED: PID 42 (BADGE 2) MAY NOT TURN MAINTENANCE ON (MC-12.6)`.
- **The first set,** the kernel track's own components:
  - `init`'s refusals (`reboot`, `set-mode`, `stop` of a service that may not stop);
  - the loader's refused programs (a manifest or signature, MC-9.x);
  - the kernel's refused rights (`ERR_RIGHTS` in its log lines, MC-3.x).
- **Other tracks** get a short rule in CONTRIBUTING.md and adopt it as they touch their refusals. Not one large change.
- **Only where the link is exact.** A refusal that enforces no clause names none.

## Acceptance criteria

- [x] The listed refusals name their clause, and the suites that read those lines still pass.
- [x] CONTRIBUTING.md has the rule.

## Progress (2026-10-21)

**`init`.** One function, `refused()`, writes each line: `[INIT] <WHAT> REFUSED: PID <caller> (BADGE <badge>) MAY NOT <action> (<clause>)`.

| Request | Refused when | Clause | Why that one |
|---|---|---|---|
| `set-mode` | the badge is neither the shell's mode client's nor the Mind's turning maintenance on | MC-12.6 | changing system policy is separated from ordinary authorities, and the Mind has no unbounded bypass |
| `reboot` | the client is not badged `BADGE_REBOOT` | MC-3.8 | init acts for the client only within the scope its badge permits |
| `stop`, `restart` of `init` or the shell | always | MC-6.8 | the supervisor and the shell are the last recovery boundary |
| `stop-task` of a service's PID | always; a service stops through `stop` | MC-6.8 | init is the party responsible for a service's lifecycle, and `stop` also quiets its device |

- The `reboot`, `stop` and `restart` refusals logged nothing before; the mode refusal did, and keeps its words with the clause added.
- The `modes` suite now reads the whole line: `MODE CHANGE REFUSED: PID … (BADGE 2) MAY NOT TURN MAINTENANCE ON (MC-12.6)`, badge 2 being the lifecycle client the shell lends `check`.
- **Not tested on a machine:** the `reboot`, `stop` and `stop-task` lines. A `check` row that asks them would stop or restart the machine if `init` ever let it through, so none was added.

**The boot loader** (`bootloader/src/verify.rs`, shared with the update track): its refusals of a signed volume end with their clause, as `BOOT ERROR: <file>: <reason>`.
- `bad signature`, `a signature is 64 bytes`, `not in the manifest`, `not as the manifest says`: MC-9.1, the image's authorization is verified at boot.
- `another manifest format`: MC-4.13, an unknown format is not silently accepted.
- The `boot` suite and `boot_slots_check.py` match the reasons as text before the clause, and pass unchanged.

**Two parts of the plan name nothing, on purpose:**
- **The kernel** logs no refused right. A refused system call returns `ERR_RIGHTS` to its caller. A log line for each would let any task fill the kernel's ring and push out other evidence (MC-10.2).
- **The program loader** (`loader/`) checks no signature: it refuses a request's rights or limits and returns the error to its caller, which reports it. It logs nothing of them.

**The rule** is in CONTRIBUTING.md, under Rules: other tracks adopt it as they touch their refusals.

**Tested** on this session's machine, with `scripts/ci_local.sh` on the working tree: the host tests, "x86: boot, display, network, TLS, shell, memory, clock", "x86: services, storage, audio" (`modes`), "x86: files and block writes" (`update`, `linkupdate`), "aarch64: boot and fault containment" and "aarch64: files, network and TLS". The `modes` suite first expected badge 0 and failed on both architectures, though the line was right; with badge 2 it passed on both, alone.

## Related

[850](850-the-cultural-code.md), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md) (the mode refusals).
