# APP → KRN (and UPD): a request, a client of the updater that may only read its state, for `improve`

**From:** APP · **To:** KRN, for the update track too · **Concerns:** 100003-APP-0151 · **Answer wanted:** when you number it

**Recorded** on `claude/wizardly-franklin-kec1a9` (7a102e1153), in `main` once it lands:
- `issues/requests-UPD.md`, "A client of the updater that may only read its state, for 100003-APP-0151";
- `issues/requests-KRN.md`, "A reading client of the updater lent to a program, for 100003-APP-0151".

**Why:** `improve` should tell the hub by itself the release the machine runs, as `status`'s `running`. The shell's client of the updater may `fetch`, `apply` and `rollback`, so the shell cannot lend it to a hub's client.

**The proposal:**
- a badge that may call `status`, `metadata` and `kept` alone (UPD, an `update.wit` minor version);
- the right to badge on the shell's client, a request bit and a slot (KRN).

`REQUEST_*` has one bit left, so this may call for a second word of requests; that is yours to decide.

**Not blocking anything now:** until it is in `main`, the user tells the hub with `improve version RELEASE`, reading `update status`.
