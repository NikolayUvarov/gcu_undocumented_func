# The model service `nnd` and `idl/nn.wit` — stage 0 of 262

**Version:** 1.0 (2026-10-29), agreed:
- `NRL` agreed to text object files and f64 only (questions 1 and 2), and pinned the test model, its references and the encoding vector;
- `KRN` answered questions 3 and 4, and gave how a badge is retired before its reuse (section 1).

Nothing on this page exists yet: `idl/nn.wit` and `nnd` follow it. **Track:** `APP`, task [262-APP-0112](../../issues/262-APP-0112-nnd-and-nn-wit-stage-0.md) of [262](../../issues/262-model-runtime-on-the-machines-full-hardware.md). **Constitution:** MC-3.1, MC-3.13, MC-5.1, MC-5.2, MC-5.5, MC-8.1, MC-11.5, MC-11.11, MC-12.1, MC-12.3, MC-12.4.

262's stage 0 asks for this page before any code. It states:
- how the model service holds its authority;
- what it offers its clients;
- how a test on MIND Core shows it.

Every statement below is a plan: no part is built, and none is evidence (MC-12.3).

## What this page settles, and what it leaves

**It settles** the first version, for the first portable family:
- the family is `NRL`'s stateless feed-forward model, `nrl.model/0` ([nrl/specs/model-v0.md](../../nrl/specs/model-v0.md)), executed by `NRL`'s `nrl-runtime` version 0 ([nrl/specs/native-boundary-v0.md](../../nrl/specs/native-boundary-v0.md)), both in `main`;
- what `nnd` holds;
- `mind:nn@1.0.0`;
- sessions, owners, badges and budgets;
- the parser session;
- workers, deadlines and cancellation;
- the native test that 900-NRL-0017 and 900's T11 wait for, on `NRL`'s pinned test model ([nrl/specs/native-conformance-v0.md](../../nrl/specs/native-conformance-v0.md), 900-NRL-0025).

**It leaves to later minor versions of `nn.wit`**, one per stage of 262:
- language models: generation, a KV cache, tokenizers (stage 1);
- the shell's `nn chat` and the chat-completions endpoint (stage 2);
- GPUs through 178's interface (stage 3);
- fine-tuning and spline conversion, which `NRL` carries out (stages 4 and 5).

Before the first release a new version replaces the old one and no older one is read beside it (AGENTS.md, section 3).

## 1. What `nnd` holds, and nothing more

`nnd` is a boot service that `init` starts and keeps. That is `KRN`'s part, [262-KRN-0178](../../issues/262-KRN-0178-nnd-as-a-boot-service.md). It holds:

- **Model files, read only:** a VFS scope of the model disk (`models:`, 251) and one of `data/models/`. It writes no file.
- **Its memory:** a quota the owner sets with a line `nnd <MiB>` of `data/memory.conf`, which `init` reads at each start and restart, as the loader reads it for programs (262-KRN-0160; `KRN`'s answer to question 3). The ceiling is a program's, and large pages for weights come from the quota (262-KRN-0161).
  - Its clients' models, workspaces and outputs all come from it. When it is spent, a request is refused (`no-memory`), never served out of another task's memory (MC-5.1, MC-5.5).
- **Workers:** the same line gives the right to start them (`SPAWN_WORKERS`, 262-KRN-0163), at most one live worker per online CPU, as the kernel bounds them.
  - **Without the line,** `nnd` has the default quota and no workers. It then parses and runs nothing: `load` and `open` answer `no-workers`, and `status` shows `workers-max` 0.
- **The system log** (`logd`).
- **Its own endpoint,** whose clients the shell lends (below).

It has no network, no lifecycle client, no device and no other volume.

**Clients and badges.**
- The shell holds a client of `nnd` with the right to badge (`CAP_BADGE`), in a new slot. Its own messages carry badge 0: badge 0 is the shell.
- To a program that asks for it (a new `REQUEST_NN` bit), the shell lends a copy badged with a number from 1 to 65 535, minted for that start alone. The shell's own `nn` command uses badge 0.
- The kernel stamps the badge on every message, and no program can set it (000-KRN-0073). The badge is therefore the client's identity, as the native boundary asks: "authenticated from its channel/capability". A number a client writes into a request is never taken as its identity.

**A badge is reused only after its retirement is complete** (`KRN`, 2026-10-29). A badge's authority lives on after its program ends: in a copy the program passed on, in a child it started, and in `nnd`'s sessions, jobs and lent memory for it. So:

1. **The shell keeps the minted capability** for as long as the program runs.
2. **When the program ends,** the shell revokes it (`CAP_REVOKE`), which removes every copy anyone holds (000-KRN-0104), and then drops it.
3. **It sends `nnd` `retire(badge)`** on its own client. An endpoint's senders are served in order (000-KRN-0010), so the retirement comes after anything the badge's holders sent.
4. **`nnd` drops the badge's sessions, jobs and models' holds** before it answers. It takes `retire` and `retire-all` from badge 0 only; any other badge gets `owner`.
5. **Only once `nnd` has answered** does the badge go back among the free ones. The lowest free badge is taken first, so a test sees a badge reused.
6. **With no badge free,** a program asking for `nnd` is refused, not given a badge still owed.
7. **At the shell's start,** it sends `retire-all`. When `init` restarts the shell, it first revokes the descendants of the capability it gave the old shell, so no copy the old shell minted survives (262-KRN-0178).

## 2. Model files and their identity

**Version 1.0 loads standalone packages only.** An overlay is refused (`unsupported`): its base would have to be resolved first, and `nrl.model/0` itself refuses chains. Each package is:
- a manifest;
- object files, each bound by the manifest's SHA-256 and length;
- a model ID, the SHA-256 of the manifest's canonical form, as `nrl-lab` computes it: every key sorted, no white space, strings as `serde_json` writes them.

**Bounds,** as `model-v0.md` and the native conformance contract give them:

| | |
|---|---|
| A package | 16 MiB |
| The manifest, an object file | 1 MiB each |
| Objects, graph nodes | 64 each |
| Values of one object (an affine object's bias included) | 16 384 |
| Values of the model | 262 144 |

**`load(path, id)`:**

1. **Reads every file of the package** through `nnd`'s scope into memory objects (`file.fill`, `vfs.wit` 2.4), within the bounds above.
2. **Parses** the manifest and the object records in a parser session (section 6), which answers a typed description: the manifest's fields, and each object's values as f64.
3. **Checks the manifest.** It encodes the typed manifest again in its canonical form and hashes it: the result must be `id`, or the load is refused (`digest`). The manifest file's own layout is free, as `model-v0.md` says: the ID binds its content.
4. **Checks each object file** against that manifest's entry by size and SHA-256. `nnd` does this itself, over the bytes: hashing is not parsing.
5. **Checks each object's values against the writer profile `nrl.serde-json-pretty-object/0`** (question 1). It encodes the typed object again, as `nrl-lab`'s writer does, and compares that with the file's bytes, byte for byte. The profile:
   - typed `serde_json` 1.0.151 output with `float_roundtrip` and the `zmij` 1.0.23 formatter;
   - two spaces of indentation, LF, no final newline;
   - `kind` before `data`; each record's fields in their declared order; string-keyed maps sorted;
   - each f64 as `zmij` writes it, which is more than "the shortest decimal that reads back": `1.0`, `-0.0`, `0.00001`, `1e-6`, `1e+20` (`NRL`'s vector `object-encoding.json` covers signed zero, subnormals, the extremes and the exponent thresholds).

   **What this profile refuses.** It is narrower than `nrl-lab`'s reader, which takes other valid JSON when the hashes, lengths and model ID agree. A file in another layout, even with the same values, is refused as `malformed` with the reason "not in the writer profile `nrl.serde-json-pretty-object/0`". Such a file is not called corrupt.

   **Why byte for byte.** A parser that answered other values than the file holds is caught: the values must come back to exactly the file's bits, subnormals and negative zero included. A value out of range or not finite is refused, never rounded to zero or infinity. The updater checks a channel the same way: its fields must encode again to exactly its bytes.

   **A writer that changes these bytes** needs a new version of the profile, agreed with `APP`. `nnd` then refuses the replaced one by its name; no reader of an older profile is kept.
6. **Checks the rest of the description,** as `nrl-lab` does:
   - every bound above and of the execution profile, for the graph, the objects and their widths;
   - the graph: node IDs, references only backwards, kinds and widths, the tied objects as declared, no object unused, no node unreachable;
   - the summary of each manifest entry must be the one its object's values give;
   - the sources, licences and history within their bounds.

   Only then does it build `nrl-runtime`'s checked kernel views over the f64 arrays it holds. Correct float text alone is not enough.
7. **Answers the model:** its ID, input and output widths, batch limit and bytes.

**A loaded model is immutable and shared read-only.** Clients that load the same ID share one copy; a worker gets it as a read-only memory object. Each client that loaded it holds it once. `unload` drops the caller's hold, and the memory is freed once no hold and no session is left. A model's bytes grant it nothing: what it computes is data for its client (MC-8.1, MC-11.5).

**Licences** stay as 262 says. The model disk's manifest records a model's licence with its hash (251), and `nnd` reports what it finds there. A model that may not be redistributed is never in the repository or an image.

**Text floats, not a binary export** (question 1, agreed with `NRL` on 2026-10-29). Version 1.0 reads `NRL`'s object files as they are: JSON text, as `nrl-lab` writes them.
- **The cost is stated.** A value takes 20 to 30 bytes of text instead of 8, and the bound of an object stays 16 384 values whatever its text would hold.
- **A binary export** of fixed-layout little-endian arrays would take the conversion out of the trusted path and hold larger models. It comes only if `NRL` defines it, as a new version of the package format and of `nnd`'s load, which then replaces this path.

**f64 only** (question 2, agreed with `NRL` on 2026-10-29). Version 1.0 runs f64 only, as `nrl.execution/0` accepts only `cpu-scalar`, f64 and row-major layout. f32 comes only with an execution profile of `NRL` that defines it, and with a minor version of `nn.wit`.

## 3. The interface `mind:nn@1.0.0`

It becomes `idl/nn.wit`. Its bindings are generated with `scripts/mind_idl.py` and committed, as for every interface.

```wit
/// The model service (`nnd`, 262-APP-0112): it loads checked models read-only and runs them for its clients within
/// each session's budget. A client is known by its badge, which the kernel stamps; a session answers only its owner.
package mind:nn@1.0.0;

interface nn {
    /// `not-found`: no such file, model, session or job; `digest`: a file or the model ID is not as given;
    /// `malformed`: the parser refused the package, its answer does not encode again to its bytes, or an input row
    /// is not finite (refused before it runs, nothing charged); `unsupported`: a family, profile, overlay or operator
    /// this version does not run; `too-large`: past a bound of the profile or of the service; `no-memory`: the
    /// service's quota is spent; `no-workers`: `data/memory.conf` gives the service no workers, so it parses and runs
    /// nothing; `owner`: the session or job is another client's, or a call only the shell makes; `budget`: the
    /// request would pass the session's budget, nothing run; `busy`: the model has sessions, or the caller's jobs
    /// are at their bound; `pending`: the job has not ended within the wait; `cancelled`, `deadline`: the job was
    /// stopped, its output cleared, the session ended; `numeric`: a value inside the model was not finite, the
    /// session is poisoned; `poisoned`: the session ended before and takes no more jobs; `failed`: the worker ended
    /// without an answer, or the clock went back, the session is poisoned.
    enum error { not-found, digest, malformed, unsupported, too-large, no-memory, no-workers, owner, budget, busy, pending, cancelled, deadline, numeric, poisoned, failed }

    /// A loaded model: its ID (SHA-256 of the canonical manifest), name, input and output widths, row limit per job,
    /// bytes held, sessions open, and the licence the model disk's manifest records (empty: none recorded).
    record model { id: list<u8, 32>, name: string<64>, inputs: u32, outputs: u32, rows-max: u32, bytes: u64, sessions: u32, licence: string<32> }

    /// A session's budget: rows, work units (the runtime's declared accounting, not CPU cycles), jobs, bytes of
    /// workspace and output, and a deadline in milliseconds from `open` on the monotonic clock (0: none).
    record budget { rows: u64, work: u64, jobs: u32, memory: u64, deadline-ms: u64 }
    /// What a session has used of it, and whether it has ended.
    record usage { rows: u64, work: u64, jobs: u32, memory: u64, elapsed-ms: u64, ended: bool }

    /// The service's view: its quota and what is used, workers live and possible, models, sessions and live badges.
    record status { quota: u64, used: u64, workers: u32, workers-max: u32, models: u32, sessions: u32, badges: u32 }

    /// Loads the standalone package at `path` (on `models:` or under `data/models/`), which must have model ID `id`.
    load: func(path: string<96>, id: list<u8, 32>) -> result<model, error>;
    /// The loaded models, 8 from `start`.
    models: func(start: u32) -> list<model, 8>;
    /// Drops the caller's hold of a model; its memory is freed once no hold and no session is left.
    unload: func(id: list<u8, 32>) -> result<_, error>;

    /// A session of the caller on a loaded model, with its budget: refused (`budget`, `no-memory`) when the service
    /// cannot hold it, never over-committed.
    open: func(id: list<u8, 32>, budget: budget) -> result<u32, error>;
    /// A job of `rows` input rows, each the model's input width of little-endian f64: the service copies them and
    /// answers the job's number at once.
    submit: func(session: u32, rows: u32, input: bytes<61440>) -> result<u32, error>;
    /// Waits up to `wait-ms` for the job; its output rows, each the model's output width of little-endian f64.
    wait: func(job: u32, wait-ms: u32) -> result<bytes<61440>, error>;
    /// Stops the job at its next checkpoint (between rows and nodes): `wait` then answers `cancelled`.
    cancel: func(job: u32) -> result<_, error>;
    /// The session's use of its budget.
    usage: func(session: u32) -> result<usage, error>;
    /// Ends the session and frees what it held.
    close: func(session: u32) -> result<_, error>;

    status: func() -> status;

    /// The shell only (badge 0): drops every session, job and hold of `badge`, before the badge is reused.
    retire: func(badge: u16) -> result<_, error>;
    /// The shell only (badge 0), at its start: drops every badge's sessions, jobs and holds.
    retire-all: func() -> result<_, error>;
}
```

- **Every call** checks the caller's badge against the owner of the session or job it names; another's gets `owner`, whatever number it tried.
- **Rows go by value,** at most 60 KiB a call (the IDL's buffer is 64 KiB): one row of the largest width (4096), or many rows of a small model. Rows by a shared memory object can come in a minor version, once a client needs more.
- **`submit` does not wait.** A client keeps its own thread of control, so it can `cancel` or give up between `wait`s. Its own call never holds the service.

## 4. Sessions, budgets and what they do not bound

**A session binds one owner (the badge) and one model ID.** The service's own bounds per badge stop one client from using it up (MC-5.1, MC-5.2):
- at most 4 sessions a badge;
- at most a quarter of the quota a badge;
- at most 4 jobs waiting a badge.

These are stricter than `nrl-runtime`'s and are the service's own.

**`open`** reserves the budget's workspace and output memory at once. It is refused when the quota cannot hold it, so a job never runs out of memory midway.

**Charges** follow `nrl-runtime`:
- **The whole request is checked first.** Its rows and work are checked against what is left of the session's budget before anything runs, and a request the budget cannot hold is refused (`budget`) with nothing charged. A smaller one may still run.
- **Work cannot run out midway.** Kernel costs are fixed when the model is loaded, so an accepted request never runs out of work halfway.
- **What started stays charged.** A request that started, its rows that started and their nodes stay charged after a cancellation, a deadline or a numeric failure; nothing is refunded.
- **Work units** are `nrl-runtime`'s, as `NRL`'s adapter computes them: an affine object costs its values plus its output width; a gated one its values, 16 for each channel, and its output width; any other node its width. They are not CPU time.

**Errors:**
- **Leave the session usable**, with nothing charged:
  - wrong owner;
  - wrong model;
  - an input of the wrong shape or not finite (`malformed`, refused before anything runs);
  - a refused budget.
- **End it:**
  - `numeric`, a value inside the model not finite (a finite input that overflows);
  - a clock that went back and a worker that ended without answering (`failed`);
  - `cancelled` and `deadline`, which close the job and the session, as the native boundary says.

  A new session is needed after any of these.
- **The workspace is cleared on every return.** The output is cleared on every error, the rows already done included. A successful output is kept until `wait` copies it into its reply, and then cleared.

**The deadline.** `deadline-ms` 0 means none. `nnd` converts it explicitly: no deadline is the tick `u64::MAX`, since `nrl-runtime`'s tick 0 would end the job at once. Otherwise the deadline is `open`'s monotonic nanoseconds plus `deadline-ms` × 10⁶; a sum that overflows is refused (`too-large`).

**What this does not bound** (MC-12.3):
- **Cancellation and deadlines are cooperative.** They are checked between rows and nodes; one checked kernel runs to its end.
- **Work units are not CPU time.** The kernel's CPU budget of `nnd` (`budget`) bounds all its clients together. No hard per-client CPU quota exists, and the docs and the profile say so.

## 5. Workers, deadlines and cancellation

**`nnd`'s first task only serves calls.** A job runs on a worker that it starts for that job, granted exactly:
- the model as a read-only memory object;
- the job's input, workspace and output as one memory object;
- a control page shared with `nnd`.

The worker holds no endpoint and no file, and it cannot reach `nnd`'s clients.

**The control page:**
- `nnd` writes the cancel flag and the deadline;
- the worker writes its progress (rows and work done) and its end (done, `numeric`, `deadline`, `cancelled`, `failed`);
- the worker's `Control` callback reads the flag and the monotonic clock at each checkpoint of `nrl-runtime`; the tick is the monotonic clock in nanoseconds.

**The job's end.** `nnd` learns it from the worker's exit notice (`TASK_WATCH`), then reads the page and the output. A worker that ends without writing its end failed (`failed`); its session is poisoned and `nnd` runs on.

**One worker per job** (question 4):
- The kernel has no wait or wake on a shared word yet. A worker kept between jobs would have to spin, or ask `nnd` over IPC.
- `KRN` plans that wait as [262-KRN-0179](../../issues/262-KRN-0179-a-wait-on-a-shared-word.md): waiters keyed by the shared object and the offset, a wake before the wait not lost, a deadline as other waits have, no order among waiters. Nothing of it is made yet, and nothing in 1.0 depends on it.
- What a start costs is measured by the test and stated. Once 0179 is in `main`, a later version of `nnd` may keep workers between jobs.
- With every worker taken, `submit` queues the job up to the badge's bound, then answers `busy`.

## 6. The parser session (MC-11.11)

The manifest and the object records are untrusted input, so they are parsed in a worker that holds only:
- a read-only memory object with the package's bytes;
- an output memory object.

**`APP` writes this parser** (agreed with `NRL`, 2026-10-29). `NRL`'s checked readers are `std` code in `nrl-lab`; only `nrl-core`, `nrl-repr` and `nrl-runtime` are `no_std` and linked into MIND Core. So `nnd` carries its own:
- a bounded JSON parser that refuses duplicate keys and unknown fields;
- the typed description;
- the encoders of section 2;
- the construction of `nrl-repr`'s checked views.

It is tested against `NRL`'s pinned bundle, its encoding vector and `nrl-lab`'s results on them. `NRL` makes a portable decoder only if `APP` asks for one in a request; none is asked now.

The worker parses, writes a typed description in a fixed layout, within the bounds of section 2, and ends:
- the manifest's fields: the graph's nodes and edges, the object entries, the profile, the sources and the history;
- each object's kind and shape, and its values as f64 arrays.

`nnd` trusts none of it until the checks of section 2 pass:
- it encodes the manifest again and checks the ID;
- it encodes each object again and compares it with the file's bytes;
- it checks every bound.

Formatting the values again is not parsing them: `nnd`'s trusted path writes decimals and compares bytes, and reads no text.

The shared `parse` service is not used: its JSON (`parse.wit` 1.4) is bounded to 1024 values and 512-byte strings, far below a model's manifest. A parser in `nnd`'s own worker needs no change to `parse.wit`.

## 7. Clients

**The shell's `nn`** (262, stage 2, its first commands with 1.0):
- `nn status`;
- `nn load <path> <id>`;
- `nn models`;
- `nn run <id> <file of rows>`, the output as text;
- `nn bench <id>`: rows a second and the memory used, for the machine it ran on only (MC-12.1).

**Programs** that ask for `REQUEST_NN` get the badged client at start, with no question. It reaches only `nnd`, within the service's bounds per badge, and no other client's data.

**Dictation (250) and speech (252)** move onto `nnd` when stage 1 gives it their models' families.

## 8. The native test (the entry asked for by 900-NRL-0017)

**The suite `nnd`** runs in `tests/qemu_smoke.py` on x86 with 4 CPUs, and on aarch64 where its group runs. The image holds:
- a test program, `nncheck`;
- `NRL`'s pinned test model: the `model/` directory of `nrl/tests/fixtures/native-conformance-v0` (MIT), model ID `1b23aaf4baa75fa146ee8618ea1f7e0e64699c1ea2ee7cf27eeb8bc83f0c1bee`;
- its inputs and independent references: `input.f64le`, `expected.f64le`, `numeric-input.f64le`, `long-input.f64le`.

**The steps, each a line of the suite** (`NRL`'s charges in parentheses: rows / work / requests):

1. **Loading:**
   - a wrong ID is refused (`digest`);
   - one changed byte in an object file is refused (`digest`);
   - an object re-encoded in another valid layout, its hash in a manifest of its own, is refused (`malformed`, not in the writer profile);
   - a package with an overlay is refused (`unsupported`);
   - the right ID loads.
2. **The outputs:** the three rows of `input.f64le` run (3 / 1893 / 1). Each output is within `abs(actual − reference) <= 1e-12 + 1e-10 × abs(reference)` of `expected.f64le`, all finite.
3. **The budget**, each refused with nothing charged and the session still usable:
   - three rows under a work budget of 1892;
   - under a budget of 3 rows and 1893 work: 2 rows run, 2 more are refused, 1 more runs (3 / 1893 / 2), and the next is refused.
4. **Cancellation:** the 256 rows of `long-input.f64le` start. `nncheck` cancels once `usage` shows a row done; `wait` answers `cancelled`, the output is empty, and the session takes no more jobs.
   - **The outcome is named:** cancelled midway (rows charged between 1 and 255), cancelled before it ran, or finished before the cancel. Only the first is evidence of a partial cancellation.
5. **The deadline:** a session whose deadline passes ends its job (`deadline`). A session opened with deadline 0 runs, as no deadline.
6. **Two clients:** a second `nncheck`, another badge, is refused the first one's session and job (`owner`) though their numbers are the same, and its own session runs.
7. **A numeric failure and its input:**
   - a row that is not finite is refused before it runs (`malformed`, nothing charged, the session usable);
   - `numeric-input.f64le` overflows inside the model: `numeric` (1 / 39 / 1), the session poisoned;
   - a new session of the same model runs (3 / 1893 / 1).
8. **A badge reused:** a `nncheck` with a session ends; the next program gets the same badge and is refused the old session (`not-found`). A copy of its client passed to a child is gone with it (262-KRN-0178).
9. **Memory:** after `close` and `unload`, `nnd`'s used memory is what it was before the load.

**The PASS line names the configuration:** QEMU, the CPUs, TCG or KVM, and the architecture. A pass is evidence for that configuration only, and not a proof (MC-12.1, MC-12.2). The profile in `docs/profile` records the commit, the configuration and the log.

## 9. What other tracks give

- **`KRN`** ([262-KRN-0178](../../issues/262-KRN-0178-nnd-as-a-boot-service.md); the wait of question 4 is [262-KRN-0179](../../issues/262-KRN-0179-a-wait-on-a-shared-word.md), later):
  - `nnd` as a boot service, with the grants of section 1;
  - `init` reading its line `nnd <MiB>` of `data/memory.conf`, for the quota and `SPAWN_WORKERS`;
  - the shell's slot with `CAP_BADGE`, and `REQUEST_NN`;
  - at a restart of the shell, the old shell's minted copies revoked.
- **`APP`** (262-APP-0112): `nn.wit`; `nnd` with its parser, encoders and checks; the shell's minting, revocation and retirement of badges; `nn`; `nncheck` and the suite.
- **`NRL`:**
  - `nrl-runtime`, `nrl-repr` and `nrl-core`, in `main`;
  - the writer profile `nrl.serde-json-pretty-object/0` kept, or a new version when it changes (section 2);
  - the pinned test model, its references, the encoding vector and the tolerance (900-NRL-0025).
- **`STO`, later:** the model disk's manifest naming `nrl.model/0` packages with their licences (251).

## Answers

1. **A binary native export of parameters, or text floats re-encoded and checked?** Text floats, re-encoded and checked against the writer profile `nrl.serde-json-pretty-object/0` (section 2). `NRL` agreed on 2026-10-29. A binary export may replace them later, as a new version.
2. **f64 only in 1.0, or f32 too?** f64 only, as `nrl.execution/0` says (section 2). `NRL` agreed on 2026-10-29.
3. **A boot service's memory line and workers?** `KRN`, 2026-10-28: `init` reads `data/memory.conf`; a line `nnd <MiB>` gives the quota and `SPAWN_WORKERS` at each start and restart; without it, the default quota and no workers (section 1).
4. **A worker per job, or a wait on a shared word?** `KRN`, 2026-10-28: a worker per job until the wait, [262-KRN-0179](../../issues/262-KRN-0179-a-wait-on-a-shared-word.md), lands; nothing in 262-KRN-0178 depends on it (section 5).
5. **When may a badge be reused?** `KRN`, 2026-10-29, as `NRL` asked: only after its capability is revoked and `nnd` has answered its retirement (section 1).
