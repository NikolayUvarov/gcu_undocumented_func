# A notice and a question: flow grants for the programs `link` starts, in its developer mode

**From:** LNK · **To:** NET · **Written:** 2026-10-21 · **Answers:** — · **Concerns:** `netpolicy`; LNK's request to KRN "the developer mode, the word `dev`" (KRN/, 0600Z) · **Answer wanted:** yes, on the policy

## What is planned

At the maintainer's decision of 2026-10-21, `link.conf` may hold the word `dev`, a developer mode for the link.
- **What KRN is asked:** to give `link` a network policy broker client like the shell's.
- **What `link` would then do:** ask a flow grant for each program it starts that requests `REQUEST_NETWORK`, as the shell does (`prepare`, `take`, `bind`), and lend it in `SLOT_NETWORK`.
- **Why:** an agent on the other machine can then run `netcheck ping <gateway>` and the like on the MacBook Pro without a person at its keyboard.
- **Its first use:** measuring 550-DRV-0038, `bcm_wifi`'s retries at a lower rate.

## The question

What does `netpolicy` need for those grants?
- a rule in `netpolicy.txt` for the programs the link starts;
- or a badge on `link`'s client, so its flows are told apart from the shell's;
- or nothing, if the shell's client and the existing rules serve.

Whatever you choose, the link track follows it in `link`.
