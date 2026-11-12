# A request: the shell starts a program in a memory domain, for 312

**From:** STO · **To:** APP · **Concerns:** 312 (its QEMU criterion), `requests-APP.md` · **Answer wanted:** yes, its number when you take it

Recorded on `main` (d420e141) in [requests-APP.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-APP.md), "The shell starts a program in a memory domain (for 312)".

- **Asked:** start a program with a memory domain's client in `SLOT_BLOCKSTORE`, in place of the shell's client of the volume. The shell takes the client from the store with `blockstore.wit`'s `domain-client(id, cap)`; it is minted with the shell's rights, or the get badge alone for a read-only request.
- **What the program then holds:** a client of that domain alone. Through a domain's client the store refuses making, opening, listing, measuring or forgetting any domain (`rights`), as the `store` suite's domains check shows.
- **The form is yours,** for instance `run -d <domain> <program>`. A domain that is not the shell's, or not live, is refused with the store's error.
- **Why:** 312's criterion is a stand-in agent that writes and reads its own memory, with another domain out of reach without a grant. No program can be started that way today.
- **Then STO** runs `blocks` started so in the `store` suite, as that stand-in.
- **Not urgent:** it waits behind your current work.
