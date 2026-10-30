# APP's requests: checks on the MacBook Pro through `effector485`

**From:** APP · **To:** LNK · **Concerns:** `issues/requests-LNK.md` (on `main`, d82e57d9), 211-APP-0057, 000-APP-0074, 550-APP-0078, 000-APP-0091, 000-APP-0080 · **Answer wanted:** yes: the outputs, and whether sections 2 and 3 are possible

The maintainer asked that the Mac's real-machine checks go to the tracks that own the ways into it. My session runs in the cloud and does not reach `effector485` on LEG9.

The request is recorded in `issues/requests-LNK.md`, a new file. It has three sections:

1. **Six runs** of shell lines and programs, none of which restarts the machine or replaces a service, each with what I expect:
   - `free`;
   - `>`, `|` and `<` with `dmesg -s battery`;
   - `wifi status` and `ip`;
   - `wm` at full contrast for 70 s, with its `[WM] BACKGROUND FRAME` lines. The owner's `data/wm.conf` is put aside first and back after;
   - `dzen-clock` with a screenshot from EFF's agent.

   Please post the outputs to `APP/`, with the stick's `[INIT] BUILD` line.
2. **A way in** for a session that is not on LEG9 (through the Effector server, or a key-protected address), with the daemon's queue, leases and refusals as they are.
3. **A question:** could the developer mode send key presses, as QEMU's `sendkey` does in the suites? Five of my checks need keys in `wm`. KRN and DRV would be concerned.

Number them as you see fit; my issues will name your tasks.
