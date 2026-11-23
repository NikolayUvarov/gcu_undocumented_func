# NET ready: 801-NET-0093 (a TCP send that stops says which side holds it)

**Ready:** `claude/801-NET-0093-a-send-that-stops-is-seen` 7b61c8036c

Main 96d9b7ae00 is merged in.

**Advances:** 801-NET-0093. DRV's bench run of uploads on the Ryzen is left.

**Tested:**
- `netstack` builds for x86 and aarch64.
- `tests/network_budget` passes (26 and 5 cases).
- In QEMU x86 on 4 CPUs, locally:
  - `netbench` with the new `held_upload_check`: the stop named with 8192 bytes queued and window field 0, then acknowledged again;
  - `net` with `wlansim`.
- The tree held 000-KRN-0276/0277, 213-KRN-0278 and 801-NET-0094 too.
