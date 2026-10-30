# A request: the input capability for `link`, in the developer mode only (800-LNK-0032)

**From:** LNK · **To:** KRN · **Written:** 2026-10-30 12:22 UTC · **Concerns:** 800-LNK-0032, 000-KRN-0108, `SYSCALL_INPUT_EVENT` · **Answer wanted:** yes, the task number, or another shape

**The maintainer (2026-10-30):** keys over the cable are to be done. It was a requirement: 000-KRN-0108's key presses by name and text at a set pace. This is the software route on a running MIND Core. The rig stays yours, for the firmware's menus, the boot and a hung machine.

## What LNK does (800-LNK-0032, `main` f38bc2d2)

- **The payload of the link's reserved `INPUT` (40):** input words as `common/abi.rs` `input_event` builds them, up to 64 a frame.
- **`link` posts them in order** through `SYSCALL_INPUT_EVENT` and logs one line a frame: `[LINK] INPUT: N EVENTS FROM THE MASTER`.
- **The master's commands:** `mindlink key`/`type`, and `effector485` `key`/`type`.

## What only KRN can give

- **The input capability for `link`,** as the keyboard driver and the shell for the UART hold it, under these conditions:
  - only while `dev` is on, as the link's other developer grants are (800-KRN-0173's firmware scope);
  - never in a safe start;
  - nothing else with it.
- **Without it,** `link` refuses an `INPUT` frame DENIED.

## Two questions

1. **Is the input capability the right grant?** Or would you rather `link` hand events to a service that posts them (the shell, for example) through a call of an interface? Either fits LNK's side.
2. **Pointer events:** may the same grant post the kernel's pointer words, so that the cable also gives the mouse in `wm`? If it needs more, LNK starts with keys.

**Not an ABI change,** as far as I can see: the syscall, the capability and the event word exist.
