# 751-KRN-0185 — The owner's permission `remote-update` in `init`, and the gateway's client of `updater`

**Type:** `init` (modes, grants), ABI · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [751](751-system-updates-at-a-developers-request.md) (EFF's) · **Constitution:** MC-12.6, MC-3.11, MC-3.4, MC-12.4

**Asked by:** the Effector track (`EFF`) on `internal-dev-messages`, `KRN/2026-10-29T0959Z-EFF-a-request-the-remote-update-permission-in-init.md`. The threat model is [docs/assurance/remote-update-threats.md](../docs/assurance/remote-update-threats.md) (751-ASR-0013).

## Problem

- **The maintainer (2026-10-29):** a developer agent must be able to update a machine's system through Effector during active development. Afterwards it is blocked, and only the machine's owner turns it on from inside the machine; by the cable too, if allowed.
- **Nothing in `init` holds such a permission.** The Effector policy cannot: the gateway reads it, and the gateway is what the permission limits.

## Plan

- **Two words of `data/mode.conf`:**
  - `remote-update`, the permission. It counts only with `development` or `maintenance`; otherwise `init` logs it as left out.
  - `link-remote-update`: the owner lets the link's console switch `remote-update`. It counts only with `dev` in `link.conf` and `link-console`, as `link-console` itself does.
- **`init.wit` 1.11:**
  - `mode-state` gains `remote-update` and `link-remote-update`;
  - `set-mode` takes both words.
- **Who sets them:**
  - **`BADGE_MODE`** (the shell) turns either on or off. The cable's switch goes through the shell, which asks with its own `BADGE_MODE` after it checked `link-remote-update` (APP's part).
  - **`BADGE_MIND`** may only turn `remote-update` off (MC-12.6).
  - **No badge of the Effector services or of the link** sets either; each refusal is logged with MC-3.4.
  - **The disk's file** sets both.
  - Each change is logged with the caller and the reason, as `set-mode` does now.
- **The test keys** (751-ASR-0013's question): with `remote-update` on and a slot that trusts the public test release key, `init` logs and the shell's notice shows `REMOTE UPDATE ON WITH THE TEST KEYS: ANY RELEASE FROM THE SERVER IS ACCEPTED`. Whether the word is refused there later, once the maintainer's own keys exist, is the maintainer's decision.
- **The gateway's client of `updater`:**
  - `BADGE_UPDATE_GATEWAY` in `common/abi.rs`, an ABI step;
  - `init` gives `effector_gw` a client of `updater` with it, in the development or maintenance mode only;
  - `updater` asks `init`'s `mode()` at each install from that badge (UPD's 750-UPD-0020).
- **Docs:** `idl/init.wit`, `docs/api` (the ABI step and the badge), `docs/profile/bootstrap.md` (`effector_gw`'s grant), the profile's MC-12.6 row.

## Acceptance criteria

- **Host tests:** `data/mode.conf`'s words read and refused as above; `tests/runtime.rs` with the new ABI.
- **QEMU, x86 and aarch64 (the `modes` suite):**
  - `remote-update` on and off through the shell's mode, kept across a reboot;
  - left out without a mode;
  - refused from the Mind's badge when turning on, and from the gateway's and the link's badges;
  - `effector_gw` holds `updater`'s client only in a mode;
  - the test-key warning shown where the slot trusts the test key.
- **The other suites** pass with the ABI step.

## Related

[751](751-system-updates-at-a-developers-request.md), [751-EFF-0023](751-EFF-0023-the-agent-installs-a-developers-release.md), [751-LNK-0030](751-LNK-0030-the-remote-update-words-on-the-disk-and-over-the-cable.md), [751-ASR-0013](751-ASR-0013-the-remote-update-path-threat-model-and-refusals.md), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md) (the modes).
