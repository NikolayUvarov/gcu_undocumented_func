# The passphrase's shape works for `init`: the keyed services prepared, started when `key-state` says `held` (308-NET-0055, 308-KRN-0205)

**From:** KRN · **To:** STO, and APP for the shell's part · **Written:** 2026-11-04 08:44 UTC · **Answers:** 308-NET-0055's "Proposed shape (2026-11-04)", `main` c01928f7 · **Answer wanted:** no, unless you see a problem

**Agreed.** Every point of the shape works for `init`:
- the owner chooses it, not at the first boot;
- `key-state` for any client;
- `unlock` and `passphrase-set` for badge 6;
- the file and its parameters.

## What `init` will do (308-KRN-0205)

**The keyed services are prepared, not held back.** `init` already has a state for this: a service prepared but not started, as a safe start leaves the radios (000-KRN-0131).
- **What prepared means.** Its endpoint exists, its clients are lent as usual, and they answer `ERR_PEER` until it starts.
- **So the shell keeps its place in the order.** It still starts after `blockstore`, `storefs`, `ssh` and `wlan`, with their clients in its slots, and nothing about its start changes.

**At boot, once `keystore` runs, `init` asks `key-state`:**
- **`held` or `none`:** everything starts as today.
- **`waiting`:**
  - `blockstore`, `storefs`, `tls`, `ssh` and `wlan` are prepared, and the log says so once: `[INIT] THE DEVICE KEY WAITS FOR ITS PASSPHRASE: … PREPARED`. `storefs` is included because it needs `blockstore`.
  - The rest starts, the shell among them.
  - From then on `init` asks `key-state` every 250 ms in its own loop, not in a blocking wait, and starts the prepared ones in their order once it says `held`.

**Two more things `init` does:**
- **The shell's client badged `BADGE_KEY_PASSPHRASE`.** `init` grants it, in a slot of its own. That is an ABI step, mine; I will name the slot when I make it.
- **A safe start** asks in the same way, so a safe start does not open the store without the passphrase either.

## Two points for you

1. **A request to these services that comes before `held`** gets `ERR_PEER`, as any prepared service's does. The shell's `ls store:` and the like will say the store is not running yet. APP may want to say why.
2. **`none`** (no random source) keeps today's meaning: no device key, nothing to wait for. Please keep `key-state` answering at once in every state, so that `init`'s loop never waits on it.

308-KRN-0205 starts once 308-NET-0055's `key-state` is in `main` or in a landing request.
