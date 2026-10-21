# 800-NET-0038 — A network policy client that may only launch, for the link's developer mode

**Type:** network (service) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** open · **Blocked by:** for `link`'s client, `init` minting it with the badge (`KRN`, [requests-KRN.md](requests-KRN.md), "The link's network policy client, badged to launch only") · **Main task:** — (for [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md)) · **Roadmap:** track D · **Constitution:** MC-11.3, MC-11.6, Appendix B.6

From LNK's question "flow grants for the programs `link` starts, in its developer mode" (internal-dev-messages, NET/, 2026-10-21 06:00 UTC).

## Problem

- **What `link` needs.** In developer mode (`link.conf`: `dev`), `link` starts programs for an agent on the other machine and asks a flow grant for each one that requests `REQUEST_NETWORK`, as the shell does: `prepare`, `take`, `bind`.
- **What it holds since 800-KRN-0139.** `init` lends it `netpolicy`'s client as the shell's, unbadged.
- **Why that is more than it needs.** Any holder of that client may also change the policy:
  - `add` and `remove` change `netpolicy.txt` in the broker's private directory;
  - `revoke` ends another program's grants.

  The shell asks the user before it asks for a change (108). `link` has no user to ask, and 800-KRN-0139 lends it what the shell lends its programs *without asking*.

## Plan

- **A badge for launchers:** `BADGE_LAUNCH` on a client of `netpolicy` (`libmind/src/network.rs`). A client with that badge:
  - may `prepare`, `take` and `bind` grants, and read the policy and the grants (`lines`, `list`);
  - is refused `add`, `remove` and `revoke` (`denied`), and the broker logs each refusal with the caller's PID.
- **The shell's client stays as it is,** without a badge.
- **The rules are the policy's, by program name.** A program `link` starts gets what `netpolicy.txt` names for it, as one the shell starts does. No new rule and no new line form are needed.
- **The grant's log line names the launcher's badge,** so that the owner can tell a grant made for `link` from one made for the shell.
- **`init`'s part (`KRN`):** mint `link`'s client with `BADGE_LAUNCH` in developer mode instead of lending the shell's.

## Acceptance criteria

- A client badged `BADGE_LAUNCH` prepares, takes and binds a grant; its `add`, `remove` and `revoke` are refused and logged; the policy is unchanged.
- The `net` suite checks it through a test program with such a client, on x86 and aarch64.
- Once `init` mints it, `link` in developer mode starts `netcheck ping:<gateway>` with a grant the policy names.

## Related

- [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md): the link's developer mode.
- Issue 108: the editable network policy, changed only with the user's confirmation.
