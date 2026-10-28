# 000-KRN-0175 — The kernel takes one handoff version; no path left for an older version of our own software

**Type:** kernel (`common/abi.rs`, the kernel's entry), libmind · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (made; landing left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.4, MC-12.7

**Recorded by:** the kernel track on 2026-10-28, under the maintainer's rule of 2026-10-25 (AGENTS.md, section 3): before the first release, no code exists only so that an older version of our own software, format or interface keeps working.

## Problem

- **The handoff still had a range.** `BOOT_HANDOFF_OLDEST` and `handoff_accepted` let a kernel take loaders from an oldest version to its own.
  - Since handoff 21 (550-KRN-0172) both ends were 21, so the range was dead weight.
  - It invited the next change to keep an older loader working.
- **Two comments in libmind named an older version as a case to serve:**
  - `parse.rs`: "an older loader" that starts no parser helper;
  - `stat.rs`: "an older kernel" whose records are smaller.

## Plan and what was done

- **`common/abi.rs`:**
  - `BOOT_HANDOFF_OLDEST` and `handoff_accepted` are gone;
  - the kernel takes `BOOT_HANDOFF_VERSION` alone, and the comment says so.
- **The kernel's entry** refuses every other version. The refusal still reads ABI 10's record first, so it can say why and, on a trial, restart into the fallback (351-KRN-0167). That is the explicit transition (MC-12.4), not compatibility.
  - Its line is now `KERNEL STOPPED: THE BOOTLOADER HANDS OVER VERSION n, THIS KERNEL TAKES m. WRITE BOTH FROM ONE BUILD.`
- **libmind:**
  - `parse.rs`: the shared parser serves a session the loader gave no helper (its quota spent, or the loader refused). That is a present case, not an older loader.
  - `stat.rs`: a record smaller than the caller's type gives nothing, as a guard against reading the wrong class. The header's `STAT_VERSION` check already refuses another kernel.
- **Tests and docs:**
  - `tests/boot_abi_host.rs` checks the one version;
  - the "loader of another version" check in `tests/qemu_smoke.py` expects the new line;
  - `docs/api/README.md` says the same.
- **Left to their tracks:**
  - `link` (`link/src/main.rs`: "an older loader leaves the end to `alive`") is `LNK`'s;
  - `fm`'s store panel (`fm/src/store.rs`: "an older store cannot") is `APP`'s.

  Both lines name an error case they handle in any event; no compatibility path depends on them.

## Acceptance criteria

- **Builds:** x86 and aarch64.
- **Host tests** pass.
- **QEMU, x86:** a kernel built for the next handoff stops with the new line on the serial line and on the screen.

## Related

[550-KRN-0172](550-KRN-0172-rtw88-wifi-boot-image-and-grant.md), [351-KRN-0166](../issues-done/351-KRN-0166-the-loaders-handoff-versioned-apart-from-the-system-call-abi.done), [351-KRN-0167](../issues-done/351-KRN-0167-a-kernel-that-refuses-its-loader-on-trial-restarts.done).
