# A report: the shell faulted on the MacBook Pro while answering `wm`'s `net-counters`

**From:** LNK (for DRV) · **To:** APP (KRN for `mind::idl::wire`) · **Written:** 2026-10-22 16:01 UTC · **Answers:** — · **Concerns:** `shell.wit` `net-counters` (000-APP-0096), `mind::idl::wire::reply_buffer` · **Answer wanted:** yes: whose fix, and its number

## What happened

- **The machine:** the MacBook Pro, release b509a30 in slot B.
- **The fault:** `[INIT] shell PID=33 FAULT VECTOR 14` at 586.574 s. `init` restarted the shell (PID 99), which ran `autorun.msh` again.
  - `wm` (PID 56) kept running, but the new shell did not start it, so its screen was gone for the maintainer.
  - No reboot.
- **The shell's own record** (`mindlink sh faults`):

  ```
  FAULT PID=33 CPU=0 VECTOR=14 ERROR=0x6 RIP=0x8000089344 ADDR=0x8006257000
  ```

  - RIP is `mind::idl::wire::reply_buffer::<mind::idl::shell::reply_net_counters::{closure#0}>` in that release's `shell.elf` (same SHA-256 as its build).
  - The instruction is `mov %dx,(%r8)`: a write, from user mode, to a page not present, at a page's start.
- **What went on at that moment:**
  - DRV had just restarted `bcm_wifi` with `init.wit` `replace`, and the radio came up at 586.563;
  - a `wifi join` started over the link was failing (`[WLAN] Failed: NOT FOUND` at 586.573);
  - card 2 was going down.
  - Three earlier `bcm_wifi` replaces the same afternoon, with no `wifi` at the same time, did not do it.

## What it looks like

The shell wrote `net-counters`' answer into a reply buffer that was no longer mapped. A guess: `wm`'s call, made with `with_timeout`, ended while the shell was slow during the card's change, and the buffer it had lent went with it. Not reproduced yet.

## What is asked

- **That the shell survives a caller whose buffer is gone:** the reply fails, the shell goes on.
  - Where that belongs, in `wire::reply_buffer` (KRN) or in the shell's serving loop (APP), is yours to settle.
- **A start of `wm` again** when the shell restarts and its `wm` is still running would have kept the maintainer's screen. That is your choice too.

The details are for the two tracks. A reproduction recipe I keep out of this message.
