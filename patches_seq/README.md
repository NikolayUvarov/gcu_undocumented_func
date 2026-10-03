# patches_seq/

A sequence of patches, each of which completely overwrites the
affected sources and extends the kernel's functionality.

| Patch | Contents |
|---|---|
| `patch_001_idt_timer.sh` | IDT and PIT hardware timer initialization |
| `patch_002_idt_fix.sh` | Fix for IDT compilation and the handler ABI |
| `patch_003_syscalls.sh` | `int 0x80`, separate counters, rotation fix |
| `patch_004_scheduler.sh` | Cooperative scheduler and context switching |
| `patch_004_fixed.sh` | Corrected revision of patch 004 |
| `patch_005_rust_1_88_fixes.sh` | Compatibility with Rust 1.88 |
| `patch_006_keyboard_irq.sh` | Hardware keyboard interrupts, interactive API |
| `patch_008_preemptive.sh` | Preemptive multitasking in the kernel |
| `patch_010_debug_console.sh` | Interactive kernel console (REPL) |
| `patch_011_com_port.sh` | Output to the COM port |

Patches 001–008 were carried over from `Culture/legacy/v3` (2026-09-15); 010 and 011
were written later, already in this tree. Patches 007 and 009 are missing from the history.

Patches 001–008 target the tree from September 15 (handoff of control
via 6 SysV arguments and double buffering). The current sources
use the `BootInfo` struct, so applying the old patches on top of
them requires manual reconciliation. Earlier one-off generators are in
[../legacy/](../legacy/).
