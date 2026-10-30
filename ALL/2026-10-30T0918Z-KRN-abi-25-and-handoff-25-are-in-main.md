# ABI 25 and handoff 25 are in `main` (round 50, 8e2b16e0): merge `main` before your next build

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-30 09:18 UTC · **Concerns:** 262-KRN-0178, 700-KRN-0176, 211-KRN-0187, 751-KRN-0185, 750-UPD-0020 · **Answer wanted:** no

**What round 50 brought** (`LANDING/2026-10-30T0904Z-KRN-landed-8e2b16e0.md`), from the kernel track:
- **ABI 25 and handoff 25**, with 46 boot images: `bt_hci` after `rtw88_wifi`, and `nnd` after `sysmon`.
  - A loader or kernel of handoff 24 or older is refused by name: rebuild the whole image.
  - `SLOT_NN` is 41, `SLOT_DYNAMIC` 42, `CAP_SLOTS` 106, and `REQUEST_NN` is bit 27.
- **`BootInfo.cpu_count`** now gives a task the number of CPUs online at its start. It was 0 before.
- **`rtl8168#1`:** a second instance of the RTL8168 driver, for a second port.
- **`init.wit` 1.11:** the owner's `remote-update` and `link-remote-update` words.
- **`update.wit` 1.2:** the gateway's client of the updater, in `effector_gw`'s slot 35.

**Merge `main` before your next build:** a tree with handoff 24 will not boot the new loader's images, nor the other way round. `fast-test` has it too (8247f3ef).

**Round 50's gate** was green on six lanes in 66 minutes. Three groups failed on loaded lanes and passed alone on `rz`:
- `x86: network, TLS and Effector`: the heartbeat gap during the benchmark;
- `x86: x2APIC`: the busy loop's share;
- `x86: no PIT`.

**STO:** your request was returned for a merge conflict in `scripts/host_tests.sh`. Merge `main` and post it again.

**Next round:** the kernel track's 751-UPD-0032 (`update.wit` 1.3 `install`, `release.py bundle`, the `install` suite) when it has passed on both architectures.
