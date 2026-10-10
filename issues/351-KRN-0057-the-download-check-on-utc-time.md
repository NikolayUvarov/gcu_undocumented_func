# 351-KRN-0057 — The download check runs its machine on UTC time, so the certificates verify in any time zone

**Type:** CI · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track C, for main task [351](351-self-update.md) · **Constitution:** MC-12.1 (evidence from the stated configuration), MC-12.2 (a test that passes or fails for a reason outside the system says nothing)

## Problem

The `net` suite's download check (`download_check` in `tests/qemu_smoke.py`, from 351-NET-0001 and 351-NET-0002) starts its machine with the default `-rtc base=localtime`. The guest's clock then shows the host's local time as if it were UTC.

The check makes its certificates with the host's `openssl` just before the run, valid from the host's UTC time. On a host behind UTC the guest's clock is earlier than that start, so the TLS client refuses the server's certificate as not valid yet. The step that verifies the server by the roots in `tlsroots.pem` then fails with `DOWNLOAD: TLS: Certificate`; the steps before it trust a pinned key, which ignores the certificate's dates.

CI's runners keep UTC, so CI never shows this. It was found on the maintainer's build machine (PCU, UTC−6), on the first run of the suites there (2026-10-10), and it stops any local gate run on a host outside UTC. The `tls` and `updater` suites already start their machines with `rtc="utc"` for this reason.

## Plan

`download_check` starts its machine with `rtc="utc"`, as the `tls` suite does.

## Acceptance criteria

1. With the host's zone set to one behind UTC (`TZ=America/Denver`), the `net` suite fails at the roots step before the change and passes after it, on x86.
2. The `net` suite passes in UTC, as in CI, on x86 and aarch64.

## Related

[351-NET-0001](../issues-done/351-NET-0001-http-downloads.done), [351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done).
