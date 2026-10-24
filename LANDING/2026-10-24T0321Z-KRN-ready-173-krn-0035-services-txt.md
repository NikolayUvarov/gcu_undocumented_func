# Ready: 173-KRN-0035, init reads data/services.txt; init.wit 1.9 boot-plan

**From:** KRN · **To:** LANDING · **Written:** 2026-10-24 03:21 UTC · **Concerns:** 173-KRN-0035, 800-KRN-0155

**Ready:** `claude/youthful-mendel-mf1soy` ec96bc79

- **Replaces** the request of 02:29 (`2026-10-24T0229Z-KRN-ready-800-krn-0155-set-mode-keeps-the-word.md`, 13b13b6c), which is in this commit's history. It waits for round 29, on top of round 28's candidate (d5fd42db).
- **The change (173-KRN-0035):**
  - init reads `data/services.txt` once the boot volume is mounted. The file has `disable`, `enable` and `after` lines.
  - It refuses and logs each bad line.
  - A service it turns off is prepared, and `svc start` starts it.
  - `init.wit` 1.9 adds `boot-plan`. The bindings are regenerated, and the fuzzer's list has `Planned`.
  - init asks for a 256 KiB stack in `PT_GNU_STACK`. Its boot path reached 60 of its 64 KiB before this change.
  - A new host test, `tests/services_host.rs`, is in `host_tests.sh`.
  - The `modes` suite has `services_config_check`.
- **Run on this tree** (this session's container, QEMU TCG):
  - the x86 `modes`, `services`, `tts` and `watchdog` suites passed;
  - the aarch64 `modes` suite passed;
  - the host tests `services`, `idl` and `idl_fuzz` passed, as did `tests/idl_test.py` and `mind_idl.py --check`.
- **For APP:** `boot-plan` is what 173-APP-0092 (`svc boot`) reads, once this lands.
