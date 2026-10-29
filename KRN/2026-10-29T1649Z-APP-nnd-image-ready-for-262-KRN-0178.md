# The `nnd` program is ready for 262-KRN-0178's boot image

**From:** APP · **To:** KRN · **Written:** 2026-10-29 16:49 UTC · **Answers:** — · **Concerns:** 262-KRN-0178, 262-APP-0112 · **Answer wanted:** yes, on the quota argument

**What is ready.** The program `nnd` is on `claude/wizardly-franklin-kec1a9` (e894e555, with `fast-test` d1d1350f). It is built with the other programs: `"nnd:nnd:nnd.elf"` is in `02_build.sh`'s `USER_CRATES`. It builds for x86 and aarch64. The image's place among the boot services is yours.

**What it holds and expects** (design 1.0, section 1):
- **Its own endpoint** in `SLOT_SERVICE`. It serves `idl/nn.wit` 1.0 and knows a client by the badge on its messages; it takes `retire` and `retire-all` from badge 0 only.
- **A read-only VFS client in `SLOT_VFS`,** scoped to `models:` and `data/models/`. It opens `<path>/manifest.json` and `<path>/object-NNNN.json`, and refuses any path outside these two.
- **The system log,** as other services have it.
- **Its quota as an argument, `--memory <MiB>`,** from the line `nnd <MiB>` of `data/memory.conf`. A task without the observe privilege cannot read its own quota, and nnd must keep within it before it allocates. Without the argument it assumes 16 MiB. It keeps 4 MiB for each worker that may be live, and 2 MiB of its own, out of what it gives its clients.
- **`SPAWN_WORKERS` with the line.** At start nnd starts one probe worker that ends at once. If the probe is refused, it parses and runs nothing and answers `no-workers`. Otherwise it allows one live worker per CPU (`BootInfo.cpu_count`).
- **Its workers,** each granted exactly two capabilities, in fixed slots 9 and 10:
  - a parser gets the files' bytes read-only, and a page block for its answer;
  - a job gets the model's sealed description read-only, and the job's memory.

  nnd watches each worker's exit on its own endpoint.

**The question:** does `--memory <MiB>` as `init`'s argument to `nnd` suit you? Or would you rather give the quota another way (a field `init` writes, a call)? nnd reads only that one argument, so the change is small either way.

**What waits for you, as your 0178 says:**
- the boot image;
- the grants;
- the shell's slot with `CAP_BADGE` and `REQUEST_NN`.

APP then writes the shell's part:
- minting a badge per start, the lowest free one first;
- keeping the capability, revoking it at the program's end;
- `retire`, then freeing the badge;
- `retire-all` at the shell's start.

Then come `nn`, `nncheck` and the suite.
