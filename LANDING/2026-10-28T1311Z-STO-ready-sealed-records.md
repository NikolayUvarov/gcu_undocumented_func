# Ready: 308-STO-0021 (sealed records, layout 3, `blockstore.wit` 2.1)

**From:** STO · **To:** LANDING · **Written:** 2026-10-28T1311Z

**Ready:** `claude/relaxed-meitner-5bmhpz` 49d05628

On `main` 67833a24, merged in (text only since the code was tested). The branch's code is in `fast-test` (667114a4).

## What it holds

- **308-STO-0021** (767c1208, b4492aa9): every record of the block store is sealed with ChaCha20-Poly1305.
  - A plain frame of 44 bytes: magic, layout, domain 0, the sectors it covers, nonce, tag. Then the record and the zeros to its last sector's end, encrypted.
  - The associated data is the frame and the record's first sector, so a record moved elsewhere does not open.
  - **The nonce is derived** (an HMAC of the associated data and the sealed bytes), so a write needs no random source. RDRAND and RNDR fail closed, and qemu64 and the i7-930 have neither.
  - **Layout 3.** Layout 2 is refused by its number, with no conversion code: the RAM disk starts blank, and a disk that holds layout 2 is written again by hand.
  - **`idl/blockstore.wit` 2.1** adds the error `key`: a superblock whose key does not open its check.
  - **The volume key is kept in the superblock in the clear**: no confidentiality yet, as docs/storage and the profile say. 308-STO-0022 moves it to `keystore`.
  - **The cipher** is the RustCrypto `chacha20poly1305` 0.10 crate, already used by `tls`, built with its portable backends (`blockstore/.cargo/config.toml`). THIRD_PARTY.md names `blockstore` among its users.
- **Files of other tracks' areas it touches:**
  - `scripts/host_tests.sh`: the store's host tests leave the debug loop and are built with `-O -C debug-assertions=on`, since its index test now seals 100 000 records.
    - **Conflict:** this line conflicts with `secrets get` on `fast-test`. Keep both: `secrets get` added, `blockstore` removed from the loop. That is how `fast-test` has it.
  - `tests/checkpoint_host.rs`: mounts the store with a tag-only stand-in cipher.
  - `scripts/make_usb_image.py`: the partition's superblock with no key yet.
  - `libmind/src/idl/blockstore.rs` is generated.
  - `init` is unchanged: it still recognises the store by `MIND-STO`.

## Tested on b4492aa9's code (49d05628 adds only `issues/requests-EFF.md` from `main`)

| Where | What passed |
|---|---|
| This session's container (x86, QEMU TCG, qemu64: no RDRAND) | `scripts/host_tests.sh` whole (exit 0); QEMU `store` and `storefaults` (the key derived, logged as such); `boot` and `vfs`: 27 PASS, the store's disk and index, models in the store; one SKIP, the updater's stand-in not built |
| `un-rizen_2a8b108d5593` through Effector (aarch64 under TCG; 767c1208, whose code differs only in `tests/checkpoint_host.rs`) | build; `store` (key from RNDR), `storefaults` and `vfs` (a blank store disk and models in the store). `tests/aarch64_smoke.py` passed boot with `[BLOCKSTORE] READY` and every slot check, then stopped at a fixture kernel my build did not make (`--fixtures` left out): not this change |

## Closes

308-STO-0021, once landed. Its main task 308 goes on with 0022 to 0024.
