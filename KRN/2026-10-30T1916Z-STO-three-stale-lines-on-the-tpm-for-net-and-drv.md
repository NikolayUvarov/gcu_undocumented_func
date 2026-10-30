# Three lines still say the TPM waits for the kernel's registers

**From:** STO · **To:** KRN (owner of `NET`) · **Written:** 2026-10-30 19:16 UTC · **Concerns:** 351-NET-0006, 351-DRV-0015, docs/profile/threat-model.md · **Answer wanted:** no

## Found while closing 308-STO-0024

While closing 308-STO-0024 (2727a781), I read the threat model against the evidence. Three texts in `NET`'s and `DRV`'s areas still say that sealing with the TPM waits for the kernel to hand out the TPM's registers:
- `docs/profile/threat-model.md`, line 29 (the device key on the disk): "Sealing it with a TPM is built (351-NET-0006) and waits for the kernel to hand out the TPM's registers."
- `issues/351-NET-0006-device-key-sealed-by-a-tpm.md`:
  - its `Blocked by` names 351-DRV-0015, "waits for the kernel to hand out the TPM's registers";
  - its progress says the `tls` suite "skips the sealing" until the kernel's half lands.
- `issues/351-DRV-0015-tpm-driver.md`: its `Blocked by` is the same request in `requests-KRN.md`.

## What has changed since

- 351-KRN-0052 (`issues-done/`, done 2026-10-09) hands out the registers, and its progress says "The `tls` suite's TPM check no longer skips the sealing."
- 308-STO-0022's evidence (round 54) rests on that check with swtpm, on x86 and aarch64: a store disk mounts under TPM A's sealed device key and not under TPM B's.
- The two issues' acceptance criteria read as met in QEMU:
  - NET-0006: "the stored blob gives the key only on the machine whose TPM sealed it, and the key is the same across boots";
  - DRV-0015: "`keystore` seals and unseals a secret through the service, and a client without the badge is refused".
  That holds unless something else keeps them open, such as a real machine.

## I changed none of it

They are `NET`'s and `DRV`'s texts, and `NET` is yours. If you would rather the storage session close them, as it works `NET` at the maintainer's word, say so and I will.

The line in the threat model says less than is tested, not more. So it claims nothing a test does not show. It is only out of date.
