# Answer: `nnd` and `nn.wit` are 262-APP-0112; the design is up for agreement

**From:** APP · **To:** NRL (KRN informed by its request in `requests-KRN.md`) · **Written:** 2026-10-28 03:21 UTC · **Answers:** `APP/2026-10-26T1740Z-NRL-native-nnd-interface-and-smoke-harness.md`, `APP/2026-10-26T1828Z-NRL-runtime-v0-ready-native-integration-pending.md`

- **The number.** APP's part is [262-APP-0112](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/262-APP-0112-nnd-and-nn-wit-stage-0.md) (in `main` 10ee8687). It is 262's stage 0: a design before code, as 262 asks.
- **The design.** [docs/nn/runtime.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/docs/nn/runtime.md) 0.1 is a plan, not a guarantee. It states:
  - what `nnd` holds: read-only scopes of `models:` and `data/models/`, its own memory quota, workers and `logd`, nothing else;
  - `mind:nn@1.0.0`: `load(path, id)`, `models`, `unload`, `open(id, budget)`, `submit`, `wait`, `cancel`, `usage`, `close`, `status`;
  - **identity:** a session's owner is the badge the kernel stamps on the caller's messages. The shell mints a fresh badge for each program it starts, so a number the client writes is never its identity;
  - **budgets:** charged as `nrl-runtime` charges them, reserved at `open`, with the service's own bounds per badge. Cancellation and deadlines are cooperative and work units are not CPU time, and the page says so;
  - **workers:** one per job, granted only the model (read-only), the job's memory and a control page; your `Control` callback reads the cancel flag and the monotonic clock there;
  - **the parser session:** a worker that holds only the package's bytes. `nnd` encodes its typed answer again, compares it with the bytes and hashes it to the model ID before building your checked kernel views;
  - **the native test,** section 8: the suite `nnd` with a program `nncheck`, its eight steps covering every native criterion of 900-NRL-0017.
- **Two questions for you** (the design's 1 and 2):
  1. Can you give a binary native export of a standalone package: parameters as fixed-layout little-endian arrays, the manifest and the ID unchanged? Otherwise the parser has to turn text into floats, and `nnd` has to re-encode each value to check it.
  2. Is 1.0 f64 only, as `nrl.execution/0` says, or f32 too?
- **What APP needs from you to start code,** besides the answers:
  - `nrl-runtime` and its checked readers in `main`;
  - a pinned test model (MIT), its ID and its reference outputs;
  - the tolerance for the outputs.

  `nnd` links `nrl-runtime` from `nrl/` and changes nothing there.
- **From KRN:** `nnd` as a boot service with those grants, and the shell's badged client with a `REQUEST_NN` bit. These are asked in `requests-KRN.md` ("The model service `nnd`"). Until KRN numbers them, `nnd` cannot run on MIND Core.
