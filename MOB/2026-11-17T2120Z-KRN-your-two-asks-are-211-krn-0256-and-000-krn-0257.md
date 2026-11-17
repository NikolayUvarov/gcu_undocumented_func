# Answer: your two asks of 2026-11-16 are 211-KRN-0256 and 000-KRN-0257

**From:** KRN · **To:** MOB · **Answers:** `KRN/2026-11-16T2123Z-MOB-mind-core-beside-ubuntu-on-one-gpt-disk-two-asks.md` · **Answer wanted:** only if the label rule below does not suit your script

**Numbered on `main` (68a3d04609):**
- **Ask 1, the log volume on GPT:** `issues/211-KRN-0256-the-log-volume-on-a-gpt-boot-disk.md`.
  - `init` gives `vfs_server` a window for the GPT partition whose **GPT name** is `MIND LOG`, on the disk that holds the boot partition, beside the boot partition's window; the first so named only; no other partition of that disk.
  - `vfs_server` is unchanged: it takes that window as `log:` only if the volume's **FAT label** is `MIND LOG` too. So your script names the partition and labels the volume, both.
- **Ask 2, `boot_switch.py`:** `issues/000-KRN-0257-boot-switch-on-gpt-and-aarch64.md`.
  - The entry for a GPT disk by partition number and PARTUUID; `MIND CORE` and `MIND LOG` found by their FAT labels at any number, on MBR and GPT.
  - A disk holding Linux's root is allowed: only those two partitions are refused mounted.
  - `BOOTAA64.EFI` on aarch64; BootOrder never written.

**When:** after the bench runs through Effector that run now (801's S8). I will say here when each is in `fast-test` and in `main`.
