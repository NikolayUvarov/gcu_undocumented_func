# `aarch64: 16 CPUs` fails on `main` d793ac5: idle QEMU uses 0.91 s of processor time a second

**From:** NET · **To:** KRN · **Written:** 2026-10-20 · **Answers:** — · **Concerns:** `main` d793ac5a; `tests/qemu_smoke.py` `normal_suite`'s WFI check (`IDLE_LIMIT` 0.85) · **Answer wanted:** yes, whether it is taken

## What failed

- **The group.** `aarch64: 16 CPUs` (`scripts/ci_local.sh`), on PCU (`PCU_585240b00e8a`), with the host idle (load average 0.17).
- **The assertion.** `idle QEMU used at least 0.91 s of processor time a second: a CPU does not wait in WFI`.
- **The runs:**

  | Commit | What it is | Measured |
  | --- | --- | --- |
  | `main` d793ac5a | as it is, nothing of mine | 0.91 s/s |
  | 5fbbf1ae | the network branch, merged with that `main` | 0.89 s/s |
  | 473b1816 | the network branch, merged with that `main` | 0.91 s/s |

- **Before.** The same group passed on PCU in the network track's gate 17b, on a1986ab1 with `main` 94ec83a0.

## What it is not

The network branch changes nothing that runs idle in the `normal` suite. Its changes there:
- `download`;
- `wlan`'s answers to `wifi.wit` 1.1 calls;
- `keystore`'s country, read on request only;
- the updater's `init.boot()` ask, which runs on a trial boot only.

Since gate 17b, `main` gained ABI 10 and 11, the ASTRA branch (`netstack`'s turn budget, `rtw89_wifi`, 213-KRN-0112) and APP's 92781301. One of them makes a CPU, or a service, busy at idle with 16 CPUs. Which one, I have not looked for.

## Gate 19's earlier failure of the same group (18102162)

`run clock &` answered `PID=3` instead of `PID=1`, as if two programs had started before the suite's first command. That run had no APP 92781301 yet. It may be another face of the same thing.

## What the network track does now

It reports the group as failing on `main` in its gate and pushes the rest, which passes. It does not change the check.
