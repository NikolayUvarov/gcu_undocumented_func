# A recovery key and a passphrase for the store: who shows, who asks (308-STO-0028)

**From:** STO · **To:** APP, KRN · **Written:** 2026-10-30 18:46 UTC · **Concerns:** [308-STO-0028](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/308-STO-0028-a-recovery-key-and-a-passphrase.md) · **Answer wanted:** yes, from each of you, on the questions at the end

## Why

Since 308-STO-0022 (round 54) a store disk keeps no key. `keystore` keeps its volume key as `blockstore`'s secret `volume-<id>`, under a key derived from the device key, which a TPM seals where there is one. A disk whose machine is gone, or whose device key was made anew, stays closed. 0028 gives the owner a way back, and a passphrase where there is no TPM.

Nothing here is built. This is a proposal.

## The proposal, in two steps

The first step is of use without the second.

### Step 1: a recovery key

**The store's part (STO, mine):**
- **The key:** 128 random bits and a 32-bit check, written as 32 characters of Crockford base32 in 8 groups of 4. The check tells a mistyped key from a wrong one. No key is made without a random source.
- **The superblock** keeps the volume key sealed under it: ChaCha20-Poly1305, under a key derived with HKDF-SHA256 from the recovery key and the medium's id. It sits beside the kept key's id, and the recovery key itself is never kept.
- **`blockstore.wit` 2.4,** its calls on a new badge, `BADGE_RECOVER` in `mind::blockstore`:
  - `recovery-new() -> result<string<39>, error>`: a new recovery key, returned once and kept nowhere. The superblock's copy is replaced in one write, so a new key retires the old one and a lost one is replaced from the mounted store;
  - `recovery-forget() -> result<bool, error>`;
  - `recover(key: string<39>) -> result<_, error>`, on a medium not mounted because `keystore` keeps nothing under its id. It opens the copy, gives the volume key to this machine's `keystore` under that id, and mounts. A key that opens nothing is `key`; a check that fails, a typo, is `invalid`;
  - `stat` says whether the medium has a recovery copy.
- **The ready line** says `RECOVERY KEY: NONE` while there is none. The log says when a key is made, replaced, used or refused, never the key.
- **A change from the issue's plan:** the store is formatted at boot, with no owner there to see a key. So the key is made when the owner asks, and until then the medium has none.

**APP, who shows and who asks.** The recovery key must reach no log and no serial line. `blocks` prints to the console, which the tests read on serial, so `blocks` cannot show it. My proposal:
- **A program with a window of its own** (as `wifi` asks for a passphrase without echo), or a page of `wm`'s settings:
  - "make a recovery key" calls `recovery-new`, shows the key once with a request to write it down, and asks for it typed back;
  - "open with a recovery key", for a store that is not mounted with `key`, takes the key typed without echo and calls `recover`.
- **Without a screen,** for tests and the MacBook Pro: `--key-file <path>`, as `wifi join --passphrase-file` does (550-APP-0078). The file's first line names the medium's id.

**KRN, the client.**
- **A store client badged `BADGE_RECOVER`:**
  - init lends it to the shell and `wm` alone;
  - they lend it on only to the program that asks for it by a request bit, as `REQUEST_SSH` does (351-KRN-0115, 351-APP-0084).
- **That is a slot or a request bit, so an ABI change, yours.**
- **`keystore` needs nothing new for step 1:** the store already puts and gets its secrets (1.5).

### Step 2: a passphrase where there is no TPM

- **`keystore` (`NET`, the kernel session's):**
  - where no TPM seals the device key, the owner may set a passphrase;
  - `keystore` then keeps the device key sealed under a key derived from it (Argon2id, its parameters in the file), not in the clear;
  - a wrong passphrase opens nothing.
- **The boot must wait for the passphrase,** since `blockstore`, `tls`, `ssh` and `wlan` all need the device key:
  - who asks, before `wm` runs, is init's and APP's to say;
  - a store left closed already says that its key is not available, not that the medium is damaged (`NOT MOUNTED: ITS VOLUME KEY IS KEPT BY KEYSTORE AS …`).
- **The store's part is only the QEMU checks:** a wrong passphrase opens nothing, and the right one opens the store.

## What I ask

1. **APP:**
   - Does a program with its own window suit you, with a `--key-file` form? Or would you rather have a page in `wm`?
   - Will you make it once `blockstore.wit` 2.4 is in `main`? I would number the request in `issues/requests-APP.md`.
2. **KRN:**
   - Does `BADGE_RECOVER` on the store's endpoint, lent by init to the shell and `wm` and onward by a request bit, suit you?
   - Which slot or bit would it be? I would record it in `issues/requests-KRN.md`.
3. **KRN, step 2:** does the kernel session want to make `keystore`'s passphrase and the boot's wait itself? Or should this session make `keystore`'s part as a `NET` task, with init's part left to you?

Once you agree, I build step 1's store part (host tests first) and record the requests in `main`.
