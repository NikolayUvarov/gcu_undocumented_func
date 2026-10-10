# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (2 requests waiting, 2026-10-10; the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md), FP/SIMD for programs on aarch64 [250-KRN-0056](../issues-done/250-KRN-0056-fp-simd-for-programs-on-aarch64.done), the devicetree check [210-KRN-0055](../issues-done/210-KRN-0055-the-devicetree-suite-steps-through-qmp.done)) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## A slot and a request flag for the system clipboard (000-APP-0032)

**Recorded by:** the tools track (APP), 2026-10-09, for [000-APP-0032](000-APP-0032-system-clipboard.md) (the tools plan's phase T4: the system clipboard).

### Problem

`edit` (Ctrl+C/X/V), `fm`'s command line and the shell's line each keep their own text, so nothing can be copied from one program to another. The tools track writes the clipboard: `idl/clipboard.wit` and a `clipboard` service that holds the text. It needs a way to reach the programs that ask for it.

### Plan (a proposal; the kernel track decides)

The same way as the parser service (109-KRN-0042):

- `init` starts `clipboard` and gives the shell a client.
- `SLOT_CLIPBOARD` and `REQUEST_CLIPBOARD` are added in `common/abi.rs` and `libmind::process`.
- A launcher may fill the slot in a launch session.

The shell lends its client only to a program that asks for it, and `msh` gets a `clipboard` word. What the service answers, and to whom, is in 000-APP-0032.

### Acceptance criteria

A program that asks for `REQUEST_CLIPBOARD` holds an endpoint of `clipboard` in `SLOT_CLIPBOARD`; one that does not ask holds nothing there.

## A fixed slot for the shell's command endpoint (211-APP-0040)

**Recorded by:** the tools track (APP), 2026-10-09, for [211-APP-0040](211-APP-0040-the-shells-commands-in-console.md) (the shell's commands in `wm`'s `console`, the kernel track's request for 211).

### Problem

The shell will serve `idl/shell.wit`: a client sends a command line and the shell runs it on its own authority. The endpoint goes from the shell to `wm` and from `wm` to `console` in a launch session, so both need a fixed slot to find it in. The application slots 1–29 are all named, and fixed slots end at `SLOT_DYNAMIC` (30), below which the kernel delivers capabilities into a receive slot.

### Plan (a proposal; the kernel track decides)

- `SLOT_SHELL` in `common/abi.rs` for applications, with `SLOT_DYNAMIC` moved up (the clipboard's `SLOT_CLIPBOARD`, asked for above, can come in the same change).
- Nothing in `init`: the shell makes the endpoint and lends it itself. `REQUEST_SHELL` goes into `libmind::process` with the tools track's change.

### Acceptance criteria

A launcher fills `SLOT_SHELL` in a launch session and the program holds the endpoint there; the ABI version and the kernel's tests follow the move of `SLOT_DYNAMIC`.
