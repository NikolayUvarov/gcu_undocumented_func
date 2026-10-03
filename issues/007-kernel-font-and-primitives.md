# 007 — Graphics core: procedural font and primitives

**Type:** feature · **Priority:** medium · **Status:** open
**Affects:** `kernel/` (new `gfx` module), possibly a shared crate

## Discrepancy with the handoff

Handoff: "a basic graphics core has been set up (procedural rendering of the font and primitives)". Code: `kernel/src/main.rs` draws a single circle by iterating over every pixel of the screen; there is no font, no `rect/line/circle/text` functions and no framebuffer abstraction.

Without text output no diagnostics are possible at all (exceptions in [004](004-apic-idt-interrupts.md), load errors, counters) — so this task blocks debugging of everything else.

## Plan

1. `gfx::Framebuffer { ptr, width, height, stride, format }` with `put_pixel`, `fill_rect`, `clear`; honouring `PixelFormat` from [009](009-gop-pixel-format.md).
2. A built-in 8×8 or 8×16 bitmap font (e.g. the public-domain `font8x8` as `const [[u8; 8]; 128]` in `.rodata`; the "procedural" in the handoff can reasonably be interpreted as generation from a table). Functions `draw_char`, `draw_str`, a simple cursor/console with line wrapping and scrolling.
3. `fmt::Write` for the console → `write!(console, "ticks={}", n)` without a heap (works before [003](003-bss-and-heap-allocator.md), since `core::fmt` does not require `alloc`).
4. Primitives: `line` (Bresenham), `circle` (midpoint) — replace the per-pixel screen iteration in the current kernel.
5. Double buffering — later, after the heap.

## Acceptance criteria

- A line like `MIND CORE 0.1 | 1024x768 | ticks: N` on screen.
- `panic_handler` prints the message and the panic location instead of `loop {}`.
- Drawing the circle does not iterate over the whole screen every frame.
