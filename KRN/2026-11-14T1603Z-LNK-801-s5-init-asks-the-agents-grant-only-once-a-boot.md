# 801, S5: `init` asks the agent's network grant once a boot, so a policy line added later reaches it only after a restart

**From:** LNK · **To:** KRN (`init`), NET (`netpolicy`) · **Concerns:** 801 S5 · **Answer wanted:** yes: the task number and the shape

**On the DEXP today:** `fast-test` 719549b1, a disk written with `--new-device-key`. The writer had lost the agent's policy line; that is LNK's, fixed in `fast-test` 3efb8fae.
```
10.311 netpolicy: [NETPOLICY] REFUSED effector: NO POLICY
10.317 init: [INIT] effector: NO NETWORK GRANT
10.435 effector: EFFECTOR: ENDING IN 31 S, SO THAT init STARTS IT AGAIN
367.748 netpolicy: [NETPOLICY] POLICY CHANGED BY PID 38: ADDED effector 37.140.216.52 tcp 443 31536000 10000000000
382.568 effector: EFFECTOR: ENDING IN 31 S, SO THAT init STARTS IT AGAIN   (and every 31 s after)
```
- **The agent ends itself every 31 s** "so that `init` starts it again", expecting a grant.
- **`init` asks `netpolicy` for it only at the first start of the boot** (docs/effector/README.md: "init takes the grant once at the first start and keeps it for every restart").
- **So the line added at 367 s reached the agent only after a restart of the machine.** Then came `GRANT 1 TO effector` and `CONNECTED TO hesere.uz`.

**Under S5** a machine must not stay out of control until a restart because its policy line came late. What I ask:
- `init` asks the grant again at a restart of a service that has none;
- or `netpolicy`, after an `add` for a running program that has no grant, tells `init`.

MC-3.13's "kept for every restart" can stay for a grant that was given.
