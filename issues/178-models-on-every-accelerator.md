# 178 — Models on every accelerator: the CPU first, then AMD and NVIDIA GPUs, Intel's and an ARM board's NPUs

**Type:** main task · **Owner:** `KRN` (the accelerator interface, the kernel's part), with `DRV` (the accelerator drivers), `APP` (`mind::nn` and its users) and `PRT` (the ARM board) · **Priority:** P2 · **Status:** open (plan) · **Blocked by:** — · **Roadmap:** tracks A, G and H · **Constitution:** MC-1.5, MC-3.3, MC-5.1, MC-12.1, MC-12.3

## Problem

The maintainer (2026-10-12) asked whether the GPUs and NPUs are planned for AI, and named the hardware at hand: an NVIDIA RTX, an AMD machine (with Effector from 2026-10-13), an Intel CPU with AI Boost (Intel's NPU) and an ARM board with an NPU.

**What exists:**

- **Every model runs on the CPU.** `mind::nn` interprets a model's ONNX graph from a `MINDNN01` file, with SSE and AVX2 matrix products (`libmind/src/nn`). Dictation (250) and speech (252) use it. The model disk (251) keeps the weights, each checked by its hash.
- **The kernel keeps the AVX-512 and AMX state** of each task (174-KRN-0037), but `mind::nn` does not use them yet.
- **Main task [174](174-full-use-of-pc-hardware.md)** plans the drivers: step 5 the IOMMU (Intel VT-d in progress, 174-KRN-0043), step 6 large BARs, step 7 a GPU service behind `idl/accel.wit`, step 8 the NPUs. It plans no runtime that places a model's work on them.
- **[252](252-neural-speech-synthesis.md)** leaves its large voices (Qwen3-TTS, ESpeech) aside for want of a GPU.

## Plan

| Step | Track | What |
|---|---|---|
| 0 | `KRN`, `PRT` | **Know each accelerator.** The hardware report already lists PCI GPUs and NPUs (Intel's NPU is class 12:00). Each machine at hand boots MIND once and sends `hw0001.txt`, which names the exact devices. The ARM board's NPU is in its device tree, so the board first needs MIND to boot on it (step 6). |
| 1 | `APP`, `KRN` | **The CPU, fully.** `mind::nn`'s matrix products and convolutions use AVX-512 VNNI and AMX (int8, bf16) where the CPU has them, AVX-VNNI on the Core Ultra, and NEON or SVE on aarch64; a model runs on several CPUs at once, within the program's quota. A `bench nn` group measures a dictation model on each machine. |
| 2 | `KRN` | **One accelerator interface** (`idl/accel.wit`, 174's step 7): memory objects, queues and fences; a graph handed to the service, compiled for its device, or refused with the operators the device lacks; a time and memory budget per program (MC-5.1). `mind::nn` places a graph on the device that takes it whole, else on the CPU, and says where it ran. |
| 3 | `DRV` | **The AMD GPU first** (174 step 7a): its compute queues from AMD's documentation, the MIT-licensed `amdgpu` as reference; the kernels (matrix products, convolutions, attention, element-wise) compiled on the build host by LLVM's AMDGPU target. Developed on the Effector machine inside QEMU/KVM with the GPU passed through by VFIO, so a fault never reboots the host; then booted on it directly. Its firmware is a proprietary file (AGENTS.md section 3). |
| 4 | `DRV` | **Intel's NPU (AI Boost)** (174 step 8), written from Intel's documentation and the firmware interface (Linux's `intel_vpu` is GPL). A graph for it is compiled ahead of time on the build host with Intel's NPU compiler, and the compiled blob travels with the model, its hash on the model disk. |
| 5 | `DRV` | **The NVIDIA RTX** (174 step 7b; Turing and later have GSP): the GSP firmware as a proprietary file, NVIDIA's open kernel modules as reference; the kernels compiled ahead of time on the build host (`ptxas`), no CUDA runtime on the target. |
| 6 | `PRT`, `DRV` | **The ARM board:** MIND boots on it first (a platform profile of its own, from its device tree); then its NPU, from its vendor's documentation. Whether that is open decides the order (an open mainline driver exists for some SoCs' NPUs, none for others). |
| 7 | `APP` | **The users:** dictation and speech on the fastest device each machine has, with the same results as on the CPU within a stated tolerance; 252's large voices tried on a GPU; then the system's own models (174). |

Each driver step is evidence for its own machine only (MC-12.1). A device that fails, or is taken away, sends its graphs back to the CPU and says so in the log. No driver holds DMA beyond what it was granted (174 step 5's IOMMU first, MC-1.5).

## Acceptance criteria

- **`bench nn`** prints one dictation model's time per second of audio on the CPU and on each accelerator of each machine named above, with where each part ran.
- **Dictation on the AMD GPU and on Intel's NPU** gives the same text as on the CPU for the test sentences of 250.
- **Budgets:** a program over its accelerator budget waits or is refused; another program's share is kept.
- **A device that fails** mid-run: the graph finishes on the CPU, and the log says why.

## Related

[174](174-full-use-of-pc-hardware.md) (steps 5–8), [250](250-voice-dictation.md), [251](251-model-cache-and-model-disk.md), [252](252-neural-speech-synthesis.md), [177](177-display-output.md) (the GPU for the picture), `libmind/src/nn`.
