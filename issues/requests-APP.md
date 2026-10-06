# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track, 2026-10-06

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here: the tools track turns each into an issue and removes it from this file. The file goes when it is empty.

## `caps` and `top`: name the escrow capability kind

### Problem

Since issue 170 `init` keeps the privileges it grants in escrow: a new capability kind, `CAP_KIND_ESCROW` (15), whose `rights` field in `StatCap` is the kind of the privilege inside (`common/abi.rs`). `monitor/src/text.rs` `cap_kind` does not know it, so the `caps` tool shows init's escrowed privileges as `?`. The shell's `caps` prints them as `escrow ... OF=control` (`libmind::stat::cap_name`).

### Plan

- `cap_kind(15)` → `escrow`; where a row shows the rights, show the kind inside (`escrow control`).

### Acceptance criteria

The `caps` tool shows init's escrowed privileges by name; its tests check one.

### Related

[170](../issues-done/170-supervisor-without-usable-privileges.done).

## New task numbers for the tools track

### Problem

On 2026-10-06 the maintainer replaced the per-track counters with one scheme for all tracks (TRACKS.md). The tools track's code is `APP`. Its main tasks are numbered from 250–299, and its tasks are `NNN-APP-MMMM`, with a counter of its own starting at 0001. `u015` and `u017` keep their numbers. `docs/tools/README.md` and `docs/voice/README.md` (and their Russian versions) still describe the `uNNN` counter.

### Plan

- Number new tasks the new way.
- Update the two plans in both languages.
- Move the track's branch to the new naming when convenient (`claude/APP-<name>`). The old branch name keeps working.

### Acceptance criteria

The tools plans describe the new numbering, and the next tools task is numbered `NNN-APP-0001`.

## The `pinmap` check reads its line before the line has arrived

**Recorded by:** the storage track (STO), 2026-10-06.

### Problem

`tests/qemu_smoke.py` `shell_suite` (u017):

```python
require(vm.expect("[PINMAP] READY"), "NO PIN CONTROLLER")
```

`expect` returns as soon as `[PINMAP] READY` is in the output, and pinmap writes the rest of its line (`100x37 NO PIN CONTROLLER`) after it on the serial line. Under slow emulation the rest has not arrived yet. One local run of the group "x86: keys, shell, tools" (`scripts/ci_local.sh`, TCG without KVM, 4 CPUs, branch `claude/relaxed-meitner-5bmhpz` merged with `main` @ ed08bed) failed with:

```
AssertionError: ('NO PIN CONTROLLER', 'pinmap\nSTARTED PID=8 NAME=pinmap FOREGROUND\n[PID 8] [PINMAP] READY 1')
```

The failure is in the check, not in pinmap: pinmap printed the whole line, as issue 209 makes it.

### Plan

Wait for the end of the line:

```python
vm.expect("NO PIN CONTROLLER", after="[PINMAP] READY")
```

### Acceptance criteria

The check passes however the line is split on the serial line.
