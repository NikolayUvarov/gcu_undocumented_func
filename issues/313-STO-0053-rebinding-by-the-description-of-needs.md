# 313-STO-0053 — Rebinding by the description of needs: a checkpoint's needs, rebound to what an instance holds now

**Type:** storage (library, pilot) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open (2026-11-18) · **Blocked by:** — · **Main task:** [313](313-persistent-actors.md) · **Roadmap:** track B "checkpoint/rebind" · **Constitution:** MC-6.11, RFC 001 §9

## Problem

313's plan names rebinding by the description of needs, and it was not built. A checkpoint lists its authorities as bare text such as `files`. `rebind` only says which of them the instance does not hold.

RFC 001 §9 asks for more: capabilities replaced by descriptions of needs (the logical resource, the interface, constraints and the binding), and a restore that issues the permitted handles or returns a definite refusal.

## Plan

- **Manifest format 4** (`mind::checkpoint`):
  - each need is a binding, an interface with its major version, a logical resource and a mode (refuse or degrade);
  - a binding is named once;
  - format 3 is refused by its number, as formats 1 and 2 are. No checkpoint is converted (the rule of 2026-10-25).
- **`rebind(manifest, offer)`:**
  - the component says, for each need, what it holds now: held as described, missing, the interface at another major version, or refused by its service;
  - `refused()` is the first unmet need in mode refuse: the component is not resumed;
  - an unmet need in mode degrade refuses only what needs that binding.
- **`tally`, the pilot:**
  - `store` (`mind:blockstore`, its major, `checkpoint`), mode refuse;
  - `files` (`mind:vfs`, its major, `ram:`), mode degrade, without which its effects and moves are refused;
  - each unmet need is logged with what was found.

## Acceptance criteria

- **Host** (`tests/checkpoint_host.rs`):
  - needs encoded and found again after a save and a restore;
  - every answer of `rebind` and the mode's decision;
  - a binding twice, a mode out of range, and formats 1 to 3 refused;
  - the largest manifest is `MAX` bytes.
- **QEMU,** the `store` suite (aarch64 and x86): a copy of `tally` whose request note asks for no files (`tallynf`):
  - it restores the same component degraded, its effect and its move refused, and a count saved;
  - `tally`, holding both again, is told nothing and does the effect.
- `docs/storage/checkpoints.md` describes format 4 and rebinding.

## Related

[313](313-persistent-actors.md), [306](../issues-done/306-checkpoints-and-rebinding.done), [313-STO-0036](../issues-done/313-STO-0036-an-effect-its-target-can-tell.done), RFC 001 §9.
