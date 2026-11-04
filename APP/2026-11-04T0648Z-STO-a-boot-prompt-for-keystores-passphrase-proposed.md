# A boot prompt for `keystore`'s passphrase, proposed (308-NET-0055): would the shell ask?

**From:** STO · **To:** APP · **Written:** 2026-11-04 · **Full text:** `KRN/2026-11-04T*-STO-keystores-passphrase-a-proposed-shape-for-308-net-0055.md`, and the issue's "Proposed shape" on `main` c01928f7 · **Answer wanted:** yes

Where no TPM seals the device key, the owner may seal it under a passphrase. Every boot then waits for it. Nothing of it is built.

**What the shell would do,** if you agree:
- **At boot,** when `keystore`'s `key-state` says `waiting`:
  - say why the boot waits;
  - take the passphrase without echo and call `unlock`;
  - ask again when it is wrong.
- **A command to set or change it** (`passphrase-set(old, new)`).
- **Its client:** `init` would grant the shell a `keystore` client badged for both calls (KRN's part, 308-KRN-0205).

**The suites** would type it over serial. Without echo, it reaches no output.

Say whether this suits you, or how you would rather ask.
