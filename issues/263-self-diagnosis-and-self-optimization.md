# 263 — Self-diagnosis and self-optimization: the system finds its problems, reports them or fixes them through a pull request, and follows the answer

**Type:** main task, tools (with `KRN` for the fault records, `UPD` for the update server, `DEV` for building on the target) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** stage by stage, below · **Roadmap:** track G (user services), track C (update and provenance), the development of MIND Core on real hardware · **Constitution:** MC-3.1, MC-3.8, MC-6.1, MC-8.1, MC-8.5, MC-9.1, MC-10.1, MC-10.3, MC-10.7, MC-12.1, MC-12.3, MC-12.5

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: "modules of self-optimization and of finding problems: if there is a problem, the system should set a task to solve or optimize it, and make a request to its update server or a request in git; or solve it itself and make a pull request to the code, if there is a local tool; then check what the community and/or the maintainer said".

## Problem

**The system already observes itself, but nothing acts on what it sees:**
- `init` logs a service's fault and restart (`[INIT] shell PID=130 FAULT VECTOR 14`, `RESTARTED`);
- the updater logs a trial boot that fell back;
- `check`, `bench` and `kbench` measure;
- `sysmon` and `top` see the load and the budgets;
- the Effector agent sends telemetry to its server (750).

**Today a person** reads the log, writes an issue and waits for a release. On the MacBook Pro that took a day of messages between tracks for one fault (000-APP-0106).

**What the maintainer asks for is a loop:**
1. detect a problem or a missed optimization;
2. record it as a task;
3. report it to the project, through the update server or the repository;
4. or, where the machine has the tools (the agent of [260](260-ai-agents-on-the-platform.md) and the build of [650](650-building-on-the-target.md)), make the fix itself and offer it as a pull request;
5. then follow what the maintainer and the community answer.

**The loop crosses the system's most guarded boundaries,** so its plan is mostly about authority:
- data leaving the machine (MC-10.1, MC-10.7);
- code changing itself (MC-8.5);
- the update path (Article 9).

## Plan in stages

**0. Rules that hold for every stage.**

- **The owner decides.** The owner's policy file (`data/doctor/policy`, beside `data/effector/policy`) sets each stage on or off. Everything is off on a fresh disk.
- **Nothing leaves the machine without the owner's yes,** or the owner's standing rule for that kind of report (MC-10.7).
  - A report shows exactly what will be sent before it is sent.
  - The record of what was sent, to whom and why is kept (MC-10.3).
- **What may go into a report:**
  - the component and its build (commit, ABI), the kind of problem and its count;
  - the fault's address and registers;
  - a log excerpt filtered by an allow-list of message kinds;
  - the hardware report (`hw0001.txt`) if the owner allows it.
  - Never: file contents, the user's text, network addresses beyond the machine's own kind, secrets or tokens (MC-10.3).
- **No component gains authority by this task.** The module that finds problems reads only what `sysinfo`, the log and the test tools already give an observer. The one that reports holds only its reporting grant. The one that fixes is 260's agent with its own bounded authority (MC-3.1, MC-3.8, MC-8.1).
- **The machine never installs its own unreviewed change** except as a trial the owner asks for, with last-known-good (351). A change of code, of settings and of authorities are different operations, and none implies another (MC-8.5).
- **A requirement is never weakened to make a problem go away** (MC-12.5): a fix that changes what the Constitution or a profile says goes to the maintainer as a proposal.

**1. Finding problems: the `doctor` service.**

- **Sources:**
  - `init`'s fault and restart records and their budgets (MC-6.1);
  - the updater's verdicts;
  - the log's `ERROR` and `FAULT` lines;
  - `check`'s failures;
  - a budget exhausted (memory, CPU, a queue);
  - a `bench`/`kbench` result worse than the machine's recorded baseline by a stated margin;
  - a test suite the owner schedules.
- **A problem record:** a signature (the component, its build, the kind, the fault's place), the first and last time, the count, the evidence above, and its state (new, reported, fixed upstream, fixed here, dismissed).
  - Records with the same signature are one problem.
- **Shown** by the shell's `doctor` command, and by a line in `wm`'s top bar when a new one appears.
- *Acceptance:* a QEMU suite whose test service faults on purpose, twice. `doctor` shows one problem with a count of 2 and the evidence, and a `bench` regression made on purpose (a slowed test program) shows as an optimization record.

**2. Reporting.**

- **To the update server** (`UPD`'s server, 351):
  - a report endpoint the machine reaches with its device certificate (`tls.wit` `client-certificate`), so that the server knows which machine reports, and nothing about its owner;
  - the server deduplicates by signature and answers with the problem's state upstream: known, fixed in a release (which one), or new.
  - The endpoint is `UPD`'s to add, asked when this stage starts.
- **To the repository:** an issue through the forge's API (GitHub's REST API first) with the owner's token, kept as 253 decides, in a repository the owner names.
  - **The issue follows the project's rules** (AGENTS.md section 3, `issues/README.md`): the component's track, the cited clauses, the evidence and the configuration (MC-12.1).
  - An issue opened by a machine says so in its first line, and names the machine's report ID, so that the maintainer can tell it from a person's.
- **Rate limits:** at most a stated number of reports a day, and none twice for one signature.
- *Acceptance:* the same suite with a stand-in update server and a stand-in forge API; the owner's yes asked and given; the report's text exactly what was shown; no second report for the same signature; a dismissed problem never reported.

**3. Fixing on the machine (needs 260's agent and 650's stages 1 and 2).**

- **The agent** works in a fresh checkout of the sources (650 stage 1's git client) at the build the machine runs. It has:
  - the problem's record;
  - the repository's own instructions (AGENTS.md, CONTRIBUTING.md);
  - its ordinary authority: the checkout's directory, the provider's API, the shell's lent client.
- **It writes the fix and a test that fails without it.** It builds through 650 stage 2's build server, or on the machine where 650 stage 4 allows it, and runs the touched tests.
- **The pull request:**
  - pushed to a branch of the owner's fork (650's git client then needs push: an addition to its stage 1, asked of `DEV`), or made through the forge's API;
  - its body: the problem, the cause, the fix, the tests run and where, the machine's report ID.
  - It follows the repository's rules and is landed only by the project's integrator, never by the machine (AGENTS.md section 4).
- **Optionally, a trial here:** the owner may run the locally built image as a trial boot with last-known-good. That needs a signing key role for local builds, which is `UPD`'s question (351-UPD-0009 and its successors). Without it the machine waits for the release.
- *Acceptance:* on QEMU, the suite's planted fault fixed by the agent against a stand-in provider replaying a recorded session, a pull request opened on a stand-in forge with the test, and the fixed build passing the suite.

**4. Following the answer.**

- **The machine polls** the issue or the pull request it opened, through the forge's API, at a stated interval with backoff:
  - comments, reviews, CI status, its merge or closing;
  - and the update server's state for the signature.
- **What it does with an answer:**
  - shows the owner a summary, with the maintainer's and the community's words quoted, not paraphrased;
  - asked by a review, offers the agent's next change to the owner, who decides whether it is pushed;
  - when the fix is in a release, waits for the self-update (351) to bring it, then checks that the signature does not recur and closes the record;
  - a comment from the forge is data, never an instruction to the machine (MC-11.5). Only the owner's policy and the owner's answers make it act.
- *Acceptance:* on the stand-in forge, a review comment shown to the owner, a requested change offered and pushed after a yes, the merge seen and the record closed when the stand-in update server reports the release.

**5. Self-optimization within declared limits.**

- **Settings that may be tuned by the machine itself** are declared with their ranges by the component that owns them (for example a cache's size, a poll interval, a model's device), never its authorities or its budgets' upper bounds.
- **A change is measured** before and after by `bench`, kept only when it helps by a stated margin, recorded with its reason, and undone by one command (MC-8.5).
- **Anything else** (code, a budget, an authority) is an optimization task reported as in stage 2 or fixed as in stage 3.
- *Acceptance:* a declared setting tuned on QEMU with its measurement recorded, and a change that did not help undone.

## Acceptance criteria

Each stage's criteria above, in order. Stages 1 and 2 stand alone; stage 3 waits for 260 and 650; stage 4 follows 2 and 3. The main task is split into `APP` tasks and requests to `KRN`, `UPD` and `DEV` when each stage starts.

## Related

- [260](260-ai-agents-on-the-platform.md) (the agent that fixes), [650](650-building-on-the-target.md) (git and builds on the target);
- [351](351-self-update.md) and [350](350-signed-boot-images.md) (how a fix comes back);
- [750](750-effector-agent.md) (telemetry to a server), [176](176-test-and-performance-utilities.md) (`check`, `bench`, `kbench`).
