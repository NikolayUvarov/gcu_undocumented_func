# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track and the storage track, 2026-10-06

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here: the tools track turns each into an issue and removes it from this file. The file goes when it is empty.

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
