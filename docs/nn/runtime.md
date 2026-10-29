# The model service `nnd` and `idl/nn.wit` — stage 0 of 262

**Version:** 0.2 (2026-10-29): `KRN` answered questions 3 and 4; `APP` settled questions 1 and 2, which `NRL` has not answered, and `NRL` may still object (the answers at the end); nothing on this page exists yet · **Track:** `APP`, task [262-APP-0112](../../issues/262-APP-0112-nnd-and-nn-wit-stage-0.md) of [262](../../issues/262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.1, MC-3.13, MC-5.1, MC-5.2, MC-5.5, MC-8.1, MC-11.5, MC-11.11, MC-12.1, MC-12.3, MC-12.4

262's stage 0 asks for this page before any code. It states how the model service holds its authority, what it offers its clients, and how a test on MIND Core shows it. Every statement below is a plan: no part is built, and none is evidence (MC-12.3).

## What this page settles, and what it leaves

**It settles** the first version, for the first portable family:
- the family is `NRL`'s stateless feed-forward model, `nrl.model/0` ([nrl/specs/model-v0.md](../../nrl/specs/model-v0.md)), executed by `NRL`'s `nrl-runtime` version 0 ([nrl/specs/native-boundary-v0.md](../../nrl/specs/native-boundary-v0.md)), both in `main`;
- what `nnd` holds;
- `mind:nn@1.0.0`;
- sessions, owners and budgets;
- the parser session;
- workers, deadlines and cancellation;
- the native test that 900-NRL-0017 and 900's T11 wait for.

**It leaves to later minor versions of `nn.wit`**, one per stage of 262:
- language models: generation, a KV cache, tokenizers (stage 1);
- the shell's `nn chat` and the chat-completions endpoint (stage 2);
- GPUs through 178's interface (stage 3);
- fine-tuning and spline conversion, which `NRL` carries out (stages 4 and 5).

Before the first release a new version replaces the old one and no older one is read beside it (AGENTS.md, section 3).

## 1. What `nnd` holds, and nothing more

`nnd` is a boot service that `init` starts and keeps. That is `KRN`'s part, [262-KRN-0178](../../issues/262-KRN-0178-nnd-as-a-boot-service.md) (asked 2026-10-28; it waits for this page's agreement). It holds:

- **Model files, read only:** a VFS scope of the model disk (`models:`, 251) and one of `data/models/`. It writes no file.
- **Its memory:** a quota the owner sets with a line `nnd <MiB>` of `data/memory.conf`, which `init` reads at each start and restart, as the loader reads it for programs (262-KRN-0160; `KRN`'s answer to question 3). The ceiling is a program's, and large pages for weights come from the quota (262-KRN-0161).
  - Its clients' models, workspaces and outputs all come from it. When it is spent, a request is refused (`no-memory`), never served out of another task's memory (MC-5.1, MC-5.5).
- **Workers:** the same line gives the right to start them (`SPAWN_WORKERS`, 262-KRN-0163), at most one live worker per online CPU, as the kernel bounds them.
  - **Without the line,** `nnd` has the default quota and no workers. It then parses and runs nothing: `load` and `open` answer `no-workers`, and `status` shows `workers-max` 0.
- **The system log** (`logd`).
- **Its own endpoint,** whose clients the shell lends (below).

It has no network, no lifecycle client, no device and no other volume.

**Clients.**
- The shell holds a client of `nnd` with the right to badge (`CAP_BADGE`), in a new slot.
- To a program that asks for it (a new `REQUEST_NN` bit), the shell lends a copy badged with a number it mints for that start alone.
- The shell's own `nn` command uses its own badge.
- The kernel stamps the badge on every message, and no program can set it (000-KRN-0073). The badge is therefore the client's identity, as the native boundary asks: "authenticated from its channel/capability". A number a client writes into a request is never taken as its identity.
- Badges are 16 bits. The shell does not reuse a badge while a program that held it runs.

## 2. Model files and their identity

**Version 1.0 loads standalone packages only.** An overlay is refused (`unsupported`): its base would have to be resolved first, and `nrl.model/0` itself refuses chains. Each package is:
- a manifest;
- object files, each bound by the manifest's SHA-256 and length;
- a model ID, the SHA-256 of the manifest normalized as `model-v0.md` says.

**`load(path, id)`:**

1. **Reads every file of the package** through `nnd`'s scope into memory objects (`file.fill`, `vfs.wit` 2.4), within the bounds of `model-v0.md`: 16 MiB a package, 1 MiB a manifest or an object file.
2. **Parses** the manifest and the object records in a parser session (section 6), which answers a typed description: the manifest's fields, and each object's values as f64.
3. **Checks the manifest.** It encodes the typed manifest again into the normalized form of `model-v0.md` (compact UTF-8 JSON, keys sorted) and hashes it: the result must be `id`, or the load is refused (`digest`). The manifest file's own layout is free, as `model-v0.md` says: the ID binds its content.
4. **Checks each object file** against that manifest's entry by size and SHA-256. `nnd` does this itself, over the bytes: hashing is not parsing.
5. **Checks each object's values** (question 1). It encodes the typed record again in the layout `nrl-lab` writes, and compares that with the file's bytes, byte for byte; any difference is refused (`malformed`). The layout is `serde_json`'s pretty form of `{kind,data}`:
   - fields in their declared order, maps sorted, two spaces of indentation;
   - each f64 in its shortest decimal that reads back to the same value.

   So a parser that answered other values than the file holds is caught, and a file in another layout is refused even with the same values. The updater checks a channel the same way: its fields must encode again to exactly its bytes.
6. **Checks the rest of the description:**
   - every bound of `model-v0.md` and of the profile must hold, for the graph, the objects and their widths;
   - the summary of each manifest entry must be the one its object's values give;
   - only then does it build `nrl-runtime`'s checked kernel views over the f64 arrays it holds.
7. **Answers the model:** its ID, input and output widths, batch limit and bytes.

**A loaded model is immutable and shared read-only.** Clients that load the same ID share one copy; a worker gets it as a read-only memory object. A model's bytes grant it nothing: what it computes is data for its client (MC-8.1, MC-11.5).

**Licences** stay as 262 says. The model disk's manifest records a model's licence with its hash (251), and `nnd` reports what it finds there. A model that may not be redistributed is never in the repository or an image.

**Text floats, not a binary export** (question 1). Version 1.0 reads `NRL`'s object files as they are: JSON text, as `nrl-lab` writes them.
- **The cost is stated.** A value takes 20 to 30 bytes of text instead of 8, so an object file holds about 40 000 values within the 1 MiB that `model-v0.md` allows it, the same bound `NRL`'s own tools keep. The test model of section 8 fits it many times over.
- **A binary export** of fixed-layout little-endian arrays (`nrl.native/0`) would take the conversion out of the trusted path and hold larger models. It comes only if `NRL` defines it, as a new version of the package format and of `nnd`'s load, which then replaces this path.

**f64 only** (question 2). Version 1.0 runs f64 only, as `nrl.execution/0` accepts only `cpu-scalar`, f64 and row-major layout. f32 comes only with an execution profile of `NRL` that defines it, and with a minor version of `nn.wit`.

## 3. The interface `mind:nn@1.0.0`

A sketch, to become `idl/nn.wit` when agreed. Its bindings are generated with `scripts/mind_idl.py` and committed, as for every interface.

```wit
/// The model service (`nnd`, 262-APP-0112): it loads checked models read-only and runs them for its clients within
/// each session's budget. A client is known by its badge, which the kernel stamps; a session answers only its owner.
package mind:nn@1.0.0;

interface nn {
    /// `not-found`: no such file, model, session or job; `digest`: a file or the model ID is not as given;
    /// `malformed`: the parser refused the package, or its answer does not encode again to its bytes;
    /// `unsupported`: a family, profile, overlay or operator this version does not run; `too-large`: past a bound of
    /// the profile or of the service; `no-memory`: the service's quota is spent; `no-workers`: `data/memory.conf`
    /// gives the service no workers, so it parses and runs nothing; `owner`: the session or job is
    /// another client's; `budget`: the request would pass the session's budget, nothing run; `busy`: the model has
    /// sessions, or every worker is taken; `pending`: the job has not ended within the wait; `cancelled`, `deadline`:
    /// the job was stopped, its output cleared; `numeric`: a value was not finite or a sum overflowed, the session is
    /// poisoned; `poisoned`: the session failed before and takes no more jobs; `failed`: the worker ended without an
    /// answer, the session is poisoned.
    enum error { not-found, digest, malformed, unsupported, too-large, no-memory, no-workers, owner, budget, busy, pending, cancelled, deadline, numeric, poisoned, failed }

    /// A loaded model: its ID (SHA-256 of the normalized manifest), name, input and output widths, row limit per job,
    /// bytes held, sessions open, and the licence the model disk's manifest records (empty: none recorded).
    record model { id: list<u8, 32>, name: string<64>, inputs: u32, outputs: u32, rows-max: u32, bytes: u64, sessions: u32, licence: string<32> }

    /// A session's budget: rows, work units (the runtime's declared accounting, not CPU cycles), jobs, bytes of
    /// workspace and output, and a deadline in milliseconds from `open` on the monotonic clock (0: none).
    record budget { rows: u64, work: u64, jobs: u32, memory: u64, deadline-ms: u64 }
    /// What a session has used of it; `poisoned` after a terminal failure.
    record usage { rows: u64, work: u64, jobs: u32, memory: u64, elapsed-ms: u64, poisoned: bool }

    /// The service's view: its quota and what is used, workers live and possible, models and sessions.
    record status { quota: u64, used: u64, workers: u32, workers-max: u32, models: u32, sessions: u32 }

    /// Loads the standalone package at `path` (on `models:` or under `data/models/`), which must have model ID `id`.
    load: func(path: string<96>, id: list<u8, 32>) -> result<model, error>;
    /// The loaded models, 8 from `start`.
    models: func(start: u32) -> list<model, 8>;
    /// Unloads a model with no session open (`busy` otherwise); only the client that loaded it, or the shell.
    unload: func(id: list<u8, 32>) -> result<_, error>;

    /// A session of the caller on a loaded model, with its budget: refused (`budget`, `no-memory`) when the service
    /// cannot hold it, never over-committed.
    open: func(id: list<u8, 32>, budget: budget) -> result<u32, error>;
    /// A job of `rows` input rows, each the model's input width of little-endian f64: the service copies them and
    /// answers the job's number at once.
    submit: func(session: u32, rows: u32, input: bytes<65536>) -> result<u32, error>;
    /// Waits up to `wait-ms` for the job; its output rows, each the model's output width of little-endian f64.
    wait: func(job: u32, wait-ms: u32) -> result<bytes<65536>, error>;
    /// Stops the job at its next checkpoint (between rows and nodes): `wait` then answers `cancelled`.
    cancel: func(job: u32) -> result<_, error>;
    /// The session's use of its budget.
    usage: func(session: u32) -> result<usage, error>;
    /// Ends the session and frees what it held.
    close: func(session: u32) -> result<_, error>;

    status: func() -> status;
}
```

- **Every call** checks the caller's badge against the owner of the session or job it names; another's gets `owner`, whatever number it tried.
- **Rows go by value**, at most 64 KiB a call: two rows of the largest width (4096), or many rows of a small model. Rows by a shared memory object can come in a minor version, once a client needs more.
- **`submit` does not wait.** A client keeps its own thread of control, so it can `cancel` or give up between `wait`s. Its own call never holds the service.

## 4. Sessions, budgets and what they do not bound

**A session binds one owner (the badge) and one model ID.** The service's own bounds per badge stop one client from using it up (MC-5.1, MC-5.2):
- at most 4 sessions a badge;
- at most a stated share of the quota a badge;
- at most a stated number of jobs waiting a badge.

**`open`** reserves the budget's workspace and output memory at once. It is refused when the quota cannot hold it, so a job never runs out of memory midway.

**Charges** follow `nrl-runtime`:
- a node's work is charged before it runs;
- a row or job that started stays charged after a failure or cancellation;
- a request the budget cannot hold is refused before anything runs (`budget`).

**Errors:**
- **Leave the session usable:** wrong owner, wrong model, a malformed input, a refused budget.
- **End it:** `numeric`, a clock that went back, and a worker that ended without answering, which poison the session; `cancelled` and `deadline`, which close the job and the session, as the native boundary says. A new session is needed after either.
- **Output and workspace are cleared** on every error and every return.

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
- the worker writes its progress (rows and work done) and its end (done, `numeric`, `deadline`, `cancelled`);
- the worker's `Control` callback reads the flag and the monotonic clock at each checkpoint of `nrl-runtime`.

**The job's end.** `nnd` learns it from the worker's exit notice (`TASK_WATCH`), then reads the page and the output. A worker that ends without writing its end failed (`failed`); its session is poisoned and `nnd` runs on.

**One worker per job** (question 4):
- The kernel has no wait or wake on a shared word yet (ABI 19). A worker kept between jobs would have to spin, or ask `nnd` over IPC.
- `KRN` plans that wait as [262-KRN-0179](../../issues/262-KRN-0179-a-wait-on-a-shared-word.md): waiters keyed by the shared object and the offset, a wake before the wait not lost, a deadline as other waits have, no order among waiters. Nothing of it is made yet, and nothing in 1.0 depends on it.
- What a start costs is measured by the test and stated. Once 0179 is in `main`, a later version of `nnd` may keep workers between jobs.
- With every worker taken, `submit` queues the job up to the badge's bound, then answers `busy`.

## 6. The parser session (MC-11.11)

The manifest and the object records are untrusted input, so they are parsed in a worker that holds only:
- a read-only memory object with the package's bytes;
- an output memory object.

The worker parses with `NRL`'s checked readers, writes a typed description in a fixed layout, within the profile's bounds, and ends:
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
- a standalone model generated on the host by `NRL`'s tool at a pinned commit, under the MIT licence, with its ID and its reference outputs. `NRL` provides them.

**The steps, each a line of the suite:**

1. **Loading:**
   - a wrong ID is refused (`digest`);
   - one changed byte in an object file is refused (`digest`);
   - a package with an overlay is refused (`unsupported`);
   - the right ID loads.
2. **The outputs:** rows run, and they equal the host's reference within the tolerance `NRL` states (exactly, where the arithmetic is the same).
3. **The budget:** a job past the work budget is refused before it runs; one that spends it midway is stopped, its rows charged and its output cleared.
4. **Cancellation:** a long job is cancelled; `wait` answers `cancelled`; the session takes no more jobs.
5. **The deadline:** a session's deadline ends its job.
6. **Two clients:** a second `nncheck`, another badge, is refused the first one's session and job (`owner`), and its own session runs.
7. **A numeric failure:** an input that overflows gives `numeric`; that session is poisoned, and a new session of the same model runs.
8. **Memory:** after `close` and `unload`, `nnd`'s used memory is what it was before the load.

**The PASS line names the configuration:** QEMU, the CPUs, TCG or KVM, and the architecture. A pass is evidence for that configuration only, and not a proof (MC-12.1, MC-12.2).

## 9. What other tracks give

- **`KRN`** ([262-KRN-0178](../../issues/262-KRN-0178-nnd-as-a-boot-service.md); the wait of question 4 is [262-KRN-0179](../../issues/262-KRN-0179-a-wait-on-a-shared-word.md), later):
  - `nnd` as a boot service, with the grants of section 1;
  - `init` reading its line `nnd <MiB>` of `data/memory.conf`, for the quota and `SPAWN_WORKERS`;
  - the shell's slot with `CAP_BADGE`, and `REQUEST_NN`.
- **`NRL`:**
  - `nrl-runtime` and its checked readers, in `main` since 2026-10-28;
  - the layout of object files kept as `nrl-lab` writes it, or a new version when it changes (section 2);
  - the test model and its reference outputs, with the commit that made them;
  - the tolerance for section 8.
- **`STO`, later:** the model disk's manifest naming `nrl.model/0` packages with their licences (251).

## Answers

1. **A binary native export of parameters, or text floats re-encoded and checked?** Text floats, re-encoded and checked (section 2). A binary export may replace them later, as a new version. Settled by `APP` on 2026-10-29, since `NRL` had not answered.
2. **f64 only in 1.0, or f32 too?** f64 only, as `nrl.execution/0` says (section 2). Settled by `APP` on 2026-10-29, since `NRL` had not answered.
3. **A boot service's memory line and workers?** `KRN`, 2026-10-28: `init` reads `data/memory.conf`; a line `nnd <MiB>` gives the quota and `SPAWN_WORKERS` at each start and restart; without it, the default quota and no workers (section 1).
4. **A worker per job, or a wait on a shared word?** `KRN`, 2026-10-28: a worker per job until the wait, [262-KRN-0179](../../issues/262-KRN-0179-a-wait-on-a-shared-word.md), lands; nothing in 262-KRN-0178 depends on it (section 5).

`NRL` may object to answers 1 and 2 in a message to `APP`. The page then changes before any code depends on them. It becomes 1.0 with `idl/nn.wit`.
