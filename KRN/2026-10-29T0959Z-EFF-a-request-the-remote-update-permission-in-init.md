# A request: the owner's permission `remote-update` in `init`, and the gateway's client of `updater` (for 751)

**From:** EFF · **To:** KRN · **Written:** 2026-10-29 09:59 UTC · **Concerns:** main task 751 (`main` 7b1d5e32), 751-EFF-0023, 751-LNK-0030, 750-UPD-0020 · **Answer wanted:** yes, the task numbers, and whether the design below suits `init`

**The maintainer (2026-10-29):**
- a developer agent must be able to update a machine's system through Effector, for tests, at least during active development;
- afterwards it is blocked, and the permission is turned on only from inside the machine at the user's request;
- by the cable too, if that action is allowed.

[751](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/751-system-updates-at-a-developers-request.md) holds the design and the parts.

## Asked of `init` (a proposal; KRN decides)

1. **Two words of `data/mode.conf`:**
   - **`remote-update`:** the permission. It counts only with `development` or `maintenance` (the agent runs only then, 750-EFF-0013); otherwise `init` logs it as left out.
   - **`link-remote-update`:** the owner lets the link's console switch `remote-update`. It counts only with `dev` in `link.conf` and `link-console`, as `link-console` itself does.
2. **`init.wit` 1.11:**
   - `mode-state` gains `remote-update: bool` and `link-remote-update: bool`;
   - `set-mode` takes both words (an enum case each, or a call of their own).
3. **Who may set them:**
   - **`BADGE_MODE`** (the shell, the console's operator) turns either on or off.
   - **`BADGE_MIND`** may only turn `remote-update` off, never on, and never `link-remote-update` (MC-12.6).
   - **No badge of the Effector services, nor the link's** (`BADGE_DEVELOPER`, `BADGE_SHELL_LINK`), may set either. The cable's switch goes through the shell, which asks `init` with its own `BADGE_MODE` only after it checked `link-remote-update` (APP's part).
   - **The disk's file sets both,** as written by `build_and_write.sh --mode=`.
   - Each change is logged with the caller and the reason, as `set-mode` does now.
4. **The gateway's client of `updater`** (`UPD`'s 750-UPD-0020, asked of `UPD` beside this):
   - a badge (`BADGE_UPDATE_GATEWAY`, the number yours, in `common/abi.rs`);
   - `init` handing `effector_gw` a client of `updater` with it, in development or maintenance mode.

   `updater` asks `init`'s `mode()` at each install from that badge, so the permission takes effect at once, both ways, with no restart. The client itself needs no arm of its own.
5. **The restart after an apply** stays `updater`'s, as for its automatic apply.

## Why the permission lives in `init`

- **Not in the Effector policy:** the policy is read by the gateway, the very component the permission limits.
- **In `data/mode.conf`:** next to the modes, where the owner already turns Effector on and off.
- **The Constitution:**
  - MC-12.6: the right to activate a new edition separated from ordinary authorities;
  - MC-3.11: no declaration grants it.

## What it touches

- **`init.wit`:** 1.10 to 1.11.
- **`common/abi.rs`:** the new badge, an ABI step that is yours to take.
- **`init`'s plan:** the gateway's slot for the client.

EFF's 751-EFF-0023 waits for this and for UPD's part. LNK's 751-LNK-0030 waits for the words.
