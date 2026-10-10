# Requests for the update track (UPD), not numbered yet

**Owner:** update track · **Status:** open (2 requests waiting, 2026-10-09) · **Recorded by:** the assurance track (`ASR`), 2026-10-09

The update track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The update track turns each into a task and removes it from this file, and the file goes when it is empty.

## `release.check` raises on a signed channel it should refuse (351-ASR-0006)

**Recorded by:** the assurance track (`ASR`), 2026-10-09, from the fuzzing of [351-ASR-0006](351-ASR-0006-update-threat-model.md).

### Problem

`scripts/release.py`'s `check` promises None or a reason. For a channel file signed with the release key whose body is not what `publish` writes, it raises instead. Anyone can sign such a file with the test release key, whose seed is public. The fuzzer `tests/update_fuzz_test.py` (seed `0x351A0006`, 3 000 inputs; on `claude/351-ASR-0006-update-threats`) met seven kinds:

- `UnicodeDecodeError` from `json.loads` of a body that is not UTF-8;
- `JSONDecodeError` from one that is not JSON;
- `KeyError` from a missing field (`expires`, `version`, …);
- `TypeError` from a field of another type (`'<=' not supported between instances of 'int' and 'list'`);
- `AttributeError` from `manifests` not being an object (`'int' object has no attribute 'items'`);
- `ValueError` from an `expires` in another format (`2099-01-01`);
- `ReleaseError` from a minimum above the version (in `channel_bytes`).

Nothing is accepted that should not be: an exception refuses the channel too. But a tool that checks a server fails with a traceback instead of the reason, and the device's updater (351-UPD-0007) will parse the same format, where a parser that fails this way is worse than a refused update.

### Plan (a proposal; the update track decides)

- Parse the body in one place that answers a channel or a reason: UTF-8, JSON, each field's type and range (`version` and `minimum` integers with 1 ≤ minimum ≤ version, `expires` in its one format, `manifests` an object of architecture to 64 hex digits), then the one encoding.
- The device's parser follows the same rules, with sizes stated for every field.

### Acceptance criteria

`tests/update_fuzz_test.py` passes: every signed channel gets None or a reason, and none raises.

## The trial's count-down stops at the largest sequence number (351-ASR-0006)

**Recorded by:** the assurance track (`ASR`), 2026-10-09, from the fuzzing of [351-ASR-0006](351-ASR-0006-update-threat-model.md).

### Problem

`bootloader/src/slots.rs` (351-UPD-0006): `plan` writes the record of a trial with `sequence + 1`, and `spent` does the same.

- With `sequence` at `u64::MAX` the addition overflows. It panics in a debug build, which `tests/update_fuzz_host.rs` showed (seed `0x351A0006`).
- The bootloader's release build does not check for overflow, so the sequence wraps to 0.
- The written record then counts as older than the one followed, and the next boot follows the same record with the same tries. An unconfirmed slot keeps booting on trial and never falls back.

Reaching that number needs a crafted record (the records hold a CRC, not a signature). Whoever can write one can also mark a slot confirmed, so this is no new way in, but the count-down's guarantee (MC-9.3) does not hold at the edge.

### Plan (a proposal; the update track decides)

- Refuse a record whose sequence cannot grow (`u64::MAX`) in `Record::parse`; or
- when it is reached, write both records again from 1 with the same state.

### Acceptance criteria

`tests/update_fuzz_host.rs` passes in a debug and an optimized build: the choice never panics, and every record it writes is newer than the one it follows.
