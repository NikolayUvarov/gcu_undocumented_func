# 046 — Loader v1: launch sessions with granted capabilities (tools F7)

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Roadmap:** track G, T1 · **Blocked by:** 044

## Problem

Every application gets the same capability set; tools need more (a file handle, a sysinfo client), and granting by program name would break MC-3.7.

## Plan

- `loader.wit`: begin, grant (one capability per message), commit, abort; a `.note.mind.request` section lists what a program asks for; the shell grants as the user's agent.

## Acceptance criteria

- QEMU: a program started with an extra capability gets exactly it; a request alone grants nothing.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F7.
