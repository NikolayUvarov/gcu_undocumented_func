# Ready: the QEMU update checks run against `httpd` (110-NET-0068)

**From:** NET · **To:** LANDING · **Concerns:** 110-NET-0068 (closes it once landed), 110 (stage 1's third part)

**Ready:** `claude/110-NET-0068-update-checks-against-httpd` 1a18628c885d2d3061d0fb04e258d6563ed377ac

It contains `main` f8a5220a (round 120 recorded).

## What it brings

- **`httpd`'s test hooks,** behind the Cargo feature `test-hooks`: `--test-hooks DIR` cuts or holds the next response for a path and records each request's path and `Range`. A build without the feature refuses the flag.
- **The deadlines as flags:** `--head-timeout`, `--idle-timeout` and `--send-timeout`.
- **`scripts/serve_httpd.py`** builds `httpd` with its hooks and gives `serve_release.serve`'s interface. The `update` and `update_store` suites use it; `serve_release.py` stays for the download checks.
- **110's criterion** named `linkupdate`, which fetches nothing over HTTP; it now names `update_store`.
- **Docs:** `docs/update/server.md` and `server_RU.md` (the hooks, the deadlines), `updater.md` and `updater_RU.md`.

## Tested

- This container (4 CPUs, QEMU 8.2.2 under TCG), on 57900749 (before `main` was merged in; the merge brings APP's shell change and text only):
  - `httpd`'s host tests with and without the feature: all passed, no warning;
  - x86, the `update` and `update_store` suites against `httpd`: passed (the cut at byte 695 484, resumed from it);
  - aarch64, the same two: passed (the cut at byte 659 800).
- The gate's groups that run them: "x86: the updater and an update through the link" and "aarch64: the updater and the modes". The first build of `httpd` with its hooks in a lane's temporary directory takes about a minute there.
