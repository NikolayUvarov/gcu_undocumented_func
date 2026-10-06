# Requests for the tools track (not numbered)

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track, 2026-10-06

The tools track numbers its own issues (`uNNN`), so requests from other tracks wait here: the tools track turns each into an issue and removes it from this file. The file goes when it is empty.

## `caps` and `top`: name the escrow capability kind

### Problem

Since issue 170 `init` keeps the privileges it grants in escrow: a new capability kind, `CAP_KIND_ESCROW` (15), whose `rights` field in `StatCap` is the kind of the privilege inside (`common/abi.rs`). `monitor/src/text.rs` `cap_kind` does not know it, so the `caps` tool shows init's escrowed privileges as `?`. The shell's `caps` prints them as `escrow ... OF=control` (`libmind::stat::cap_name`).

### Plan

- `cap_kind(15)` → `escrow`; where a row shows the rights, show the kind inside (`escrow control`).

### Acceptance criteria

The `caps` tool shows init's escrowed privileges by name; its tests check one.

### Related

[170](../issues-done/170-supervisor-without-usable-privileges.done).
