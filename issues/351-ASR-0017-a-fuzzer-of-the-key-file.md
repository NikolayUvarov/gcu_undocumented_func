# 351-ASR-0017 — A fuzzer of the update's key file and of the signature lines

**Type:** assurance (host test) · **Owner:** `ASR` · **Priority:** P2 · **Status:** open (2026-11-28) · **Blocked by:** — · **Main task:** [351](351-self-update.md), for [351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done) · **Constitution:** MC-9.6, MC-12.2

## Problem

UPD asked on 2026-11-13 ([requests-ASR.md](requests-ASR.md)) for a fuzz target of `mind::release::parse_keys` and of
`parse_channel`'s signature lines (`libmind/src/release.rs`). Until now, `tests/release_host.rs` changes every byte of a
key file to nine values, and that is not a fuzzer. The harness of [351-ASR-0006](351-ASR-0006-update-threat-model.md),
named in the request, is on a branch of 2026-10-10 that never reached `main`. So the fuzzer is a host test of its own
that the host tests run.

## Plan

`tests/release_keys_fuzz_host.rs`:
- **Seeds:** what `release.py` makes:
  - key files signed by one root key, by two, and by a repeated one;
  - channel files signed by one key and by two.
- **The fuzzer:** a fixed-seed xorshift and 400 000 inputs, each made by one to four of these mutations:
  - a byte replaced by one the formats are made of, or a bit flipped;
  - bytes inserted or deleted;
  - a range repeated in place, or spliced in from another seed;
  - the input cut short;
  - a signature line repeated or dropped;
  - a digit run made longer;
  - now and then, a file past `KEYS_MAX`.

  A quarter of the inputs start again from a seed. The rest grow from the last input.
- **The invariants**, as the request names them:
  - no input panics;
  - every key file `parse_keys` takes encodes again (`encode_keys`) to exactly its body, and `keys_is` holds for it;
  - every channel `parse_channel` takes encodes again to its body, and `channel_is` holds;
  - `signed_by` counts each key once and none outside the set given. This is checked on every taken key file, and on
    20 000 made-up signature sets: over the root's keys, the signers, the signers repeated, and an outsider added, with
    checks that pass all, none, or some.
- The test asserts that more than 100 inputs of each kind were taken, so that the properties are not checked on nothing.

## Acceptance criteria

- [ ] The three cases pass: 3.9 s unoptimised, as `scripts/host_tests.sh` builds them; 0.6 s with `-O`.
- [ ] A mutation check: a `parse_keys` whose hex also takes upper-case digits is caught on its first finding, with the
      input that made it.
- [ ] All of `scripts/host_tests.sh` passes with the new file in it.
- [ ] Any finding goes to UPD, the owner of `release.rs`, as a message. This fuzzer changes no parser.

## Related

- [351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done) (the key file), [351-ASR-0006](351-ASR-0006-update-threat-model.md) (the threat model), [500](500-fuzzing-abi-and-idl.md).
