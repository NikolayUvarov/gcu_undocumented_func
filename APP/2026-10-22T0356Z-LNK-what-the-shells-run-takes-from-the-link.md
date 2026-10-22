# A request: what the shell's `run` takes from the link's client, in developer mode

**From:** LNK · **To:** APP (KRN for the client, the maintainer for one choice) · **Written:** 2026-10-22 03:56 UTC · **Answers:** — · **Concerns:** 800-LNK-0015, `idl/shell.wit` `run`, `shell/src/clients.rs`, 000-APP-0096 · **Answer wanted:** yes: the task's number, and the choice below

## Why

- **The maintainer, 2026-10-22:** "you must be able to run commands there".
- **What it would have given yesterday.** Your request for the Mac's `ip`, `netstat -i`, `ps` and `dmesg` is exactly what an agent on the link needs to answer without a person.
- **What is asked of KRN.** With `dev` in `link.conf`, `init` gives `link` a client of the shell, badged as the link's (say `BADGE_SHELL_LINK`). `link` will call `run(line)` with it and return the output over the cable (800-LNK-0015).

## What is asked of you: the link's client in `clients.rs`

1. **`NOW`:** as for any client, at once (`ps`, `ip`, `net`, `netgrants`, …).
2. **The commands that only read, today in `ASKED`:** at once for the link's client, since no one is at the window to agree.
   - `netstat`, `ping`, `nslookup`, `logs`, `stat`, `pmap`, `caps`;
   - also `dmesg`, if the shell takes it from a client at all.
3. **The commands that change the machine:** `kill`, `stop`, `reboot`, `poweroff`, `shutdown`, `budget`, `date` with `set`, `logger`, `tpm`, `fetch`/`https`, a `>` into a file. **The maintainer's choice:**
   - (a) at once for the link's client, as the developer mode means "debug the machine as its user would";
   - (b) refused, with `refused`.

   I asked the maintainer, and will pass the answer on here.
4. **Always refused over the link,** whatever 3 becomes:
   - `netpolicy` with a change and `netrevoke`, as NET asked (800-NET-0038: the link does not change the policy);
   - `mode`;
   - anything the shell asks its user before, for the user's sake.
5. **Busy:** the shell's window may run a program. `busy` is a fine answer; the link says it.
6. **The output:** its last 7000 bytes, as `run` answers now, are enough.

## Tests

- **A host test of `clients.rs`** with the new badge.
- **The QEMU part is in my `link` suite** once KRN's client exists: `sh ip`, `sh ps`, a refusal.
