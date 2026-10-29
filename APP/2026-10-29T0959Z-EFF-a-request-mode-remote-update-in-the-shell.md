# A request: `mode remote-update` and `mode link-remote-update` in the shell, and the link's console taking the first only with the second (for 751)

**From:** EFF · **To:** APP · **Written:** 2026-10-29 09:59 UTC · **Concerns:** main task 751 (`main` 7b1d5e32), 751-LNK-0030, 800-APP-0108 · **Answer wanted:** yes, the task number

**The maintainer (2026-10-29):**
- a developer agent must be able to update a machine's system through Effector, for tests, at least during active development;
- afterwards the permission is turned on only from inside the machine at the user's request;
- by the cable too, if that action is allowed.

[751](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/751-system-updates-at-a-developers-request.md) holds the design. KRN is asked for two words of `data/mode.conf`, `remote-update` and `link-remote-update`, in `init.wit` 1.11's `set-mode` and `mode-state`.

## Asked of the shell (a proposal; APP decides)

1. **At the console:**
   - `mode remote-update on|off` and `mode link-remote-update on|off`;
   - **turning either on** asks for a confirmation, as `mode` does now, naming what it allows: "a release sent through Effector may be installed".
   - **`mode` alone** shows both words.
2. **The link's console** (`Client::LinkConsole`, 800-APP-0108):
   - `mode remote-update on|off` is taken only while `init` reports `link-remote-update`, and then the shell asks `init` with its own badge;
   - without that word, it is refused as `mode` is today;
   - `mode link-remote-update …` from the link is always refused;
   - every other `mode` from the link stays refused.
3. **The notice** above the first prompt and in the system log, while `remote-update` is on: `REMOTE UPDATE: ON: A RELEASE SENT THROUGH EFFECTOR MAY BE INSTALLED`, and whether the link may switch it.
4. **`wm`'s Settings:** the two switches beside the modes, each with the same confirmation.
5. **The Mind** may only turn `remote-update` off. That is `init`'s rule, and the shell needs nothing for it.

## Tests asked

- The shell's host test, beside `the_links_console_gets_scripts_and_the_policy_but_never_mode`: the link console's rule both ways.
- The `modes` QEMU suite: the console's switch with its confirmation, and the notice.

LNK's 751-LNK-0030 tests the cable's side in the `link` suite once this and KRN's words exist.
