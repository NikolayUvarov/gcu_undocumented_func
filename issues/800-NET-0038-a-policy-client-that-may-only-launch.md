# 800-NET-0038 — A network policy client that may only launch, for the link's developer mode

**Type:** network (service) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** in progress (the broker's part in `main` through landing round 8; its QEMU check waits for `init`'s part) · **Blocked by:** for `link`'s client, `init` minting it with the badge ([800-KRN-0142](800-KRN-0142-the-links-launch-only-policy-client.md)) · **Main task:** — (for [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md)) · **Roadmap:** track D · **Constitution:** MC-11.3, MC-11.6, Appendix B.6

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

## Progress (2026-10-21)

- **Built:**
  - `idl/netpolicy.wit` 1.2 with the error `denied`;
  - `mind::network::BADGE_LAUNCH`, and `may_change_policy`, true only for a client with no badge, so an unknown badge changes nothing either;
  - `netpolicy` refuses `revoke`, `add` and `remove` from any other client and logs each refusal (`[NETPOLICY] ADD REFUSED FOR PID n: A LAUNCH-ONLY CLIENT`); a grant such a client asks for is logged `, ASKED BY A LAUNCH-ONLY CLIENT`;
  - `docs/network/policy.md` 0.4, `docs/idl`.
- **Tested on the host:** `tests/netpolicy_host.rs`.
- **Left:** the QEMU check needs a program with a launcher's client. `link` in developer mode will hold one once `init` mints it (800-KRN-0142). No program holds one before that.

## Progress (2026-10-22)

- **The broker's part is in `main`** through landing round 8 (`claude/relaxed-meitner-5bmhpz` a1488f4d, merged as 50bc8bd6; candidate aa50204e gated in full): `netpolicy.wit` 1.2 `denied`, `BADGE_LAUNCH` and `may_change_policy` in `libmind/src/network.rs`, the refusals in `netpolicy`, and `tests/netpolicy_host.rs`.
- **Left:** `init` minting `link`'s client with `BADGE_LAUNCH` ([800-KRN-0142](800-KRN-0142-the-links-launch-only-policy-client.md)), then the QEMU check.

## Related

- [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md): the link's developer mode.
- Issue 108: the editable network policy, changed only with the user's confirmation.
