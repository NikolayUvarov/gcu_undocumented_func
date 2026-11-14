# Ready: 801's S7 in the shell, `mode remote-update` (801-APP-0139) and `effector policy` (801-APP-0140)

**Ready:** `claude/wizardly-franklin-kec1a9` 8f28827de2dbf0c0b294e2dab3880226d751ce4b

It contains `main` c0f8da8b09. It is in `fast-test` (dd18dde9).

- **Closes** 801-APP-0139: `mode remote-update|link-remote-update on|off` (asked at the console, off at once). `mode` shows both. The link's console switches `remote-update` only with the owner's `link-remote-update`. The notice names the test keys.
- **Advances** 801-APP-0140: `effector policy show | allow <line> | deny <line>`.
  - The whole new policy must parse before it is written, so an edit can never leave a refused policy.
  - The file is written aside and renamed into place, and an edit a restart cut is finished or undone at the shell's start.
  - The gateway's reload at once waits for KRN's owner client (`requests-KRN.md`).
- **Tested** in this container, x86, 4 CPUs:
  - **`modes`:** 9 PASS, the new checks among them: the console's switch and refusals, the link's console over QEMU's USB serial with and without the leave, the policy's edits, a cut edit recovered;
  - **`shell`:** 17 PASS, and **`link`:** 1 PASS;
  - **host tests:** `shell_host` (41) and every other line of `scripts/host_tests.sh` pass, except three TLS tests. `tools/httpd/tests/tls.rs`'s two and `tests/effector_server_test.py`'s `test_contract_tls_pin_auth_and_prefix` fail the same way on clean `main` c0f8da8b in this container. Its Python is 3.13.18 and they are not touched here.
- **Merges** with the waiting request (KRN's 0951Z, 9622f44c) without a conflict.
