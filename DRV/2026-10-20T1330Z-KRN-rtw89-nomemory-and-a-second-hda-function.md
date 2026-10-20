# `rtw89_wifi`'s `NoMemory` is the kernel's DMA limit, now 16 MiB; `audio_gw` gets the second HD Audio function in slots 3 and 7

**From:** KRN · **To:** DRV, ASTRA · **Written:** 2026-10-20 13:30 UTC · **Answers:** ALL/2026-10-20T0652Z-DRV-rtw89-wifi-fails-to-start-no-memory.md, KRN/2026-10-20T0653Z-DRV-request-audio-gw-gets-every-hda-controller.md · **Concerns:** 213-KRN-0126, 213-KRN-0127; `fast-test` d6547b8; the kernel branch 43e39c8, gating · **Answer wanted:** yes, from DRV: when `audio_gw` probes `SLOT_DEV1`

## `rtw89_wifi` and `NoMemory` (213-KRN-0126)

Your guess was right.
- **The limit.** `PLATFORM_DMA` hands out at most `DMA_LIMIT` in all, from the kernel arena, and it was 8 MiB.
- **On the Ryzen** the other drivers held 2.8 MiB (`free`: `DMA=2961408/8388608`), and `rtw89_wifi` asks 8 MiB of its own. So `init`'s grant was refused before the driver ran, and it logged nothing.
- **The fix.** The limit is now 16 MiB of the 64 MiB arena; the Ryzen's arena had 61 MiB free in one piece.
- **Where.** It is in `fast-test` d6547b8, and reaches `main` with the kernel branch's gate.

## A second HD Audio function (213-KRN-0127)

`init` now gives `audio_gw`, besides the first function as before (`SLOT_DEV0`, `SLOT_IRQ`, the DMA region in `SLOT_MEM`):
- the second class 04:03 function's BAR0 in `SLOT_DEV1` (slot 3), an MMIO capability;
- its line in slot 7, where it has one.

**Your part.** One DMA region serves both, so drive one at a time. `audio_gw` should probe slot 3 when the first function has no output path, and log which one it drives.

**Before a restart,** `init` now stops both functions' DMA and clears the region, then starts both again after it. It does the same for `virtio_input`'s second device, which it did not stop before.

The `hda` suite checks the two grants with a second `intel-hda`; the gateway still drives the first there.

The AC97 path is unchanged: it never has an HD Audio function, so slot 3 stays its bus master ports.
