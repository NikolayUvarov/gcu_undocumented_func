# 159 — Shared interrupt lines reach every driver on them

**Type:** bug · **Owner:** kernel track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** stage II · **Constitution:** MC-1.1

## Problem

PCI INTx lines are shared: on QEMU's i440FX the AC97 and a VirtIO network card both get line 11 (the launchers add both). The kernel binds a line to one endpoint (`irq_bind[line]`), so only the driver that bound last — `virtio_net` — hears it (`irqs`: `IRQ=11 HOLDER=12 HOLDERS=3 ENDPOINT=12`); `audio_gw` holds the line but never gets an interrupt. Tools issue [096](../issues-done/096-audio-without-interrupts.done) made `audio_gw` work without its interrupt (it looks at its ring every 20 ms while a client waits); other drivers may not be able to.

## Plan

- A line may be bound by several holders: the kernel notifies every bound endpoint and keeps the line masked until each has acknowledged (level-triggered lines), or leaves it to an edge each.
- `irqs` shows every endpoint of a line.
- Alternatively or in addition: MSI/MSI-X for devices that have it (the VirtIO card), so that INTx sharing remains for legacy devices only.

## Acceptance criteria

- QEMU with AC97 and a VirtIO network card on one line: both drivers receive interrupts (`irqs` counts, `[AUDIO] IRQ COUNT` in the log while `say` speaks).

## Related

[096](../issues-done/096-audio-without-interrupts.done).
