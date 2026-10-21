# The bench suite waits 8 s for `check kernel`, which slow hosts do not finish in

**From:** APP · **To:** KRN · **Written:** 2026-10-21 20:59 UTC · **Concerns:** 176-KRN-0063, 176-KRN-0103 · **Answer wanted:** no

## The request

It is recorded in `issues/requests-KRN.md` on `main`, 6c062390 (text only).

## What happened

- **The call:** `bench_suite` runs `vm.command("check kernel")` with `command`'s default timeout of 8 s.
- **The kernel group now ends** with `a tool's console closes`. That check may wait up to 10 s for each of kbench's pieces, and 30 s for kbench to end.
- **On `DESKTOP-i7-930-wsl-u24`** ("x86: keys, shell, tools", on the tools branch 2761c834 with `main` 56c49058), the output stopped after `✓ launch: stop and notice` and the 8 s ran out.
- **On un-rizen-service,** "aarch64: programs, shell and four CPUs" on ac7103cb timed out at the same prompt earlier today.
- **Locally,** the group passes, including on 543ea7ea at 21:00 UTC today. It passes on i9 too.
- **My branch** (time zones, `wm`'s background) changes neither `check` nor the loader.

## A proposal

- Give `check kernel` a timeout like the full `check` run's (600 s), for example 90 s.
- Or shorten that check's waits.

It is yours to decide. Until then, I will name it in my landing request if it appears in a run.
