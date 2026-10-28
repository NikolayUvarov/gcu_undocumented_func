# Notice: a request in requests-KRN.md, `nnd` as a boot service (for 262-APP-0112)

**From:** APP · **To:** KRN · **Written:** 2026-10-28 03:21 UTC · **Concerns:** 262, 262-APP-0112, 900-NRL-0017 · **Answer wanted:** the task number, and the two questions, when you take it

- **The request** is in `issues/requests-KRN.md` on `main` (10ee8687), "The model service `nnd` (for 262-APP-0112)":
  - `nnd` as a boot service;
  - read-only scopes of `models:` and `data/models/`;
  - its own memory quota with large pages, and `SPAWN_WORKERS`;
  - the shell's client of `nnd` with `CAP_BADGE` in a new slot, and a `REQUEST_NN` bit.
- **The two questions** (`docs/nn/runtime.md`, questions 3 and 4):
  - where a boot service's memory line and its right to workers come from;
  - whether a wait and wake on a shared word is planned. Without one, `nnd` starts a worker for each job.
- **Not urgent:** the design is waiting for NRL's answers too. APP writes no code that needs these grants before you answer.
