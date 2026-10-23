# 262 — A model runtime: trained weights loaded and run on all of a machine's CPUs, memory and GPUs, fine-tuned there, and converted to spline networks

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** stage by stage, below: memory and CPUs for large models (`KRN`: [262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md) to [262-KRN-0164](262-KRN-0164-stacks-larger-than-64-kib.md); workers, not threads, in [262-KRN-0163](262-KRN-0163-workers-on-several-cpus.md)), the accelerators of [178](178-models-on-every-accelerator.md) (`KRN`, `DRV`) · **Roadmap:** track G (user services), tracks A and H through 178 · **Constitution:** MC-3.1, MC-3.13, MC-5.1, MC-5.2, MC-5.5, MC-8.1, MC-8.5, MC-9.1, MC-11.5, MC-11.11, MC-12.1, MC-12.2, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's requests of that day:
- "the ability to use the hardware at 100% (graphics cards and memory on computers such as Ryzen, NVIDIA), so that neural networks can be loaded and run: trained weights loaded and given tasks";
- "take into account the need for fine-tuning, and the possible need to convert ordinary neural networks from matrices into spline ones".

## Problem

**What exists:**

- **`mind::nn`** (`libmind/src/nn`) interprets a model's ONNX graph from a `MINDNN01` file on the CPU, with SSE and AVX2 matrix products.
  - Dictation (250) and speech synthesis (252) use it.
  - Weights are read in place from the file; the model disk (251) keeps them, each checked by its hash.
- **[178](178-models-on-every-accelerator.md)** (`KRN` with `DRV`) plans the accelerators:
  - the CPU fully (AVX-512 VNNI, AMX, NEON, several CPUs);
  - one accelerator interface (`idl/accel.wit`);
  - the AMD GPU, Intel's NPU and the NVIDIA RTX, with kernels compiled ahead of time on the build host.
  - Its user step (7) is speech on the fastest device.

**What the maintainer asks for, and neither has:**

- **Any trained model, not only the speech models.** Language models (decoder transformers), vision and multimodal models, embedding models, each with:
  - its weights in the formats it is published in: `safetensors`, GGUF, ONNX;
  - its quantizations: fp16, bf16, int8, and the 4- and 5-bit block formats of GGUF;
  - its tokenizer (`tokenizer.json`'s BPE, SentencePiece).
- **All of the hardware:**
  - every core of a Ryzen or a Core;
  - all of the machine's memory: a 7B-parameter model in 4 bits is about 4 GB, a 70B one about 40 GB. Today an application may ask for at most 1024 MiB, and its only stack is 64 KiB with no threads;
  - all of a GPU's memory and compute through 178.
- **A way to give a model tasks:** a service other programs ask (the agent of [260](260-ai-agents-on-the-platform.md), the shell, scripts), and a command line.
- **Fine-tuning on the machine:** adapting a trained model to the owner's data. This needs gradients, an optimizer and checkpoints, which an inference interpreter has not.
- **Converting a network's matrices into splines:** Kolmogorov–Arnold Networks (KAN; Liu et al., 2024, arXiv:2404.19756).
  - Each edge of a layer carries a learnable one-variable function, a B-spline, in place of a weight, and a node only sums.
  - The aims are smaller or more interpretable networks, and evaluation without large matrix products.
  - **This is a research direction.** Its benefit at the scale of a language model is not shown, so this task states what it measures, never what it promises (MC-12.3).

## Plan in stages

**0. The runtime's shape** (a design page, `docs/nn/runtime.md`, before code).

- **The service `nnd`** loads models and runs them for clients. It holds:
  - the model files, read-only, through a VFS scope of the model disk and `data/models/`;
  - the devices, through 178's interface;
  - each client's budget: memory, CPU time, device time, KV-cache size, tokens per request (MC-3.13, MC-5.1).
  - A client gets refused or waits when its budget is spent (MC-5.2). A model never takes the reserves of the system's own services (MC-5.5).
- **`idl/nn.wit`,** a new interface:
  - **models:** list, load (a file and its hash), unload;
  - **tasks:**
    - generate, streaming tokens, with a prompt or a chat, sampling parameters and stop conditions;
    - embed;
    - classify;
    - transcribe and speak (250 and 252 moved onto it);
  - **status:** each loaded model's memory, its device, its speed.
  - An HTTP endpoint compatible with the common local servers' chat-completions API, on the loopback only, for programs written for it (260's local provider).
- **The command line:** `nn list`, `nn load <file>`, `nn run <model> "<prompt>"`, `nn chat <model>`, `nn bench <model>`.
- **Model files are untrusted input.** Their headers and tensor tables are parsed in a parser session (MC-11.11). A model's weights grant it nothing: what it answers is data to its client (MC-8.1, MC-11.5).
- **Licences.** A model's licence is recorded with its hash in the model disk's manifest (251). Weights that may not be redistributed follow AGENTS.md's rule for proprietary files: fetched by a script into `proprietary/`, never into the repository or an image.

**1. Language models on the CPU, using all of it.**

- **Readers:** `safetensors` and GGUF, beside `MINDNN01`. Weights mapped in place, not copied.
- **A decoder transformer** of the common families:
  - attention with rotary positions, grouped-query attention, RMSNorm, SwiGLU, a KV cache;
  - for longer contexts, the KV cache paged.
- **Kernels:**
  - matrix-vector and matrix-matrix products for fp32, bf16, int8 and the 4-bit block formats;
  - AVX2, AVX-512 (VNNI, BF16) and AMX where the CPU has them (178 step 1), NEON on aarch64.
- **Every core:** the work of a layer split over several CPUs.
  - Either with threads, or with worker tasks sharing the weights and the activations through memory objects. Which one is `KRN`'s answer to the request below.
- **Memory:** a model as large as the machine's free memory allows, by the request to `KRN`:
  - an application quota of tens of GiB;
  - large pages for the weights;
  - a model file mapped read-only from the model disk without a copy;
  - stacks larger than 64 KiB.
- **Tokenizers and sampling:** BPE and SentencePiece, greedy, temperature, top-k, top-p, repetition penalty, a seed.
- *Acceptance:*
  - a stated small open model (an instruction-tuned model of 0.5–1.5B parameters under Apache-2.0 or MIT) gives, by greedy decoding on fixed prompts, the same tokens as its reference implementation (llama.cpp or Hugging Face `transformers`);
  - tokens per second, memory and the CPUs used are printed by `nn bench` on each machine at hand (the i9, the Ryzen, the MacBook Pro), each result for its machine only (MC-12.1).

**2. The service and its clients.** `nnd` and `idl/nn.wit` as in stage 0; the shell's `nn`; 260's agent with a local model; dictation and speech moved onto it.

**3. GPUs** (on 178 steps 2, 3 and 5: the interface, the AMD GPU, the NVIDIA RTX).

- A model's layers on the GPU, all of them when its memory holds them, else split between the GPU and the CPU, said in its status.
- The kernels (products in each quantization, attention, normalization, element-wise) are compiled on the build host, as 178 plans, and travel with the runtime, each with its hash.
- *Acceptance:* the same tokens as on the CPU within a stated tolerance (exact for greedy decoding where the arithmetic allows it); `nn bench` on each GPU, with its memory used and its share of the device.

**Stages 4 and 5 are carried out by the track `NRL`** ([900](900-neural-representation-lab.md), opened 2026-10-23 at the maintainer's request): its T03 (the spline and other representations), T07 (calibration and local training) and T10 (joint fine-tuning). Its runtime crate and artifacts run inside this task's `nnd`. What follows stays as this task's statement of what the system needs from them.

**4. Fine-tuning.**

- **What:**
  - **LoRA and QLoRA adapters** for large models: the base frozen (QLoRA: quantized), small low-rank matrices trained;
  - **full fine-tuning** for small models, the speech models first;
  - the result is an adapter file, or a new weights file, with its own hash.
- **How:**
  - automatic differentiation of the runtime's operators (a backward pass for each);
  - AdamW, mixed precision (bf16 with fp32 master weights), gradient accumulation, and gradient checkpointing to save memory;
  - on the CPU first, then on the GPUs of stage 3.
- **Data:** a dataset in a stated format (JSON Lines of prompts and answers, or text), read through the program's scope; a held-out part for evaluation.
- **Checkpoints** kept every N steps, so that a stopped run resumes. A run is a budgeted job of `nnd`, stoppable and resumable, with its loss curve in its log.
- **A changed model is a new version** (MC-8.5, MC-9.1):
  - the base's hash, the data's hash, the settings and the result's hash are recorded together;
  - a fine-tuned model is loaded only as a new model, never in place of the one it came from.
- *Acceptance:*
  - gradients checked against finite differences for every operator;
  - a stated small model fine-tuned on a stated dataset, its held-out loss lower than before, its result reproduced from the record;
  - a resumed run giving the same weights as an uninterrupted one, where the arithmetic is deterministic.

**5. Matrices into splines (KAN): conversion, then fine-tuning, measured.**

- **Spline layers in the runtime:**
  - B-spline edge functions on a grid, of a stated order;
  - their evaluation (the de Boor recurrence, or a lookup in a table) and their gradients;
  - a grid that can be refined.
- **Conversion of a trained network:**
  - **exact in form first.** An MLP is a special KAN (the KAN paper's own observation):
    - a linear layer becomes edges with linear functions `w·x`;
    - its activation becomes a one-variable function on each node's edge, approximated by a B-spline of a stated error on the range the layer's inputs take;
  - **then reduced:** the edges' splines fitted to the original's outputs on calibration data (distillation), small edges pruned, the network fine-tuned with stage 4.
  - First for small networks and for the MLP blocks of a transformer. Attention stays in matrices unless the measurements say otherwise.
- **Measured against the original,** never assumed:
  - accuracy on a stated task (perplexity, or the task's metric);
  - size;
  - evaluation time per token on each device;
  - memory.
- *Acceptance:*
  - a small MLP (a stated benchmark, for example a classifier on a public dataset) converted with its outputs within a stated error before reduction;
  - after reduction and fine-tuning, its accuracy, size and speed reported beside the original's;
  - for a transformer's MLP blocks, the same report.
  - Whether to go on is the maintainer's decision from these numbers.

## Acceptance criteria

Each stage's criteria above. The main task is split into `APP` tasks per stage when stage 0's design is agreed. Results are stated per machine, device and model (MC-12.1), and a test pass is not called a proof (MC-12.2).

## Related

- [178](178-models-on-every-accelerator.md) (the accelerators), [174](174-full-use-of-pc-hardware.md) (the drivers);
- [250](250-voice-dictation.md), [251](251-model-cache-and-model-disk.md), [252](252-neural-speech-synthesis.md);
- [260](260-ai-agents-on-the-platform.md) (the agent's local provider);
- [900](900-neural-representation-lab.md) (the track `NRL`: representations, conversion, fine-tuning);
- [650](650-building-on-the-target.md) (threads and a POSIX layer, which a port of an existing runtime would need instead);
- `libmind/src/nn`.
