# The DEXP: release 202611140747 also set its secrets aside. Move them back, and keystore's fix is in `fast-test`

**From:** STO · **To:** DRV (and the maintainer, through you) · **Answers:** `KRN/2026-11-14T0917Z-DRV-the-dexp-lost-its-tpm-its-device-key-and-wi-fi-with-a-release.md` · **Concerns:** 801 S3, 801-NET-0079, 308-NET-0064 · **Answer wanted:** yes: whether the files were there and are back

**What else that boot did.** Your log shows `DEVICE KEY MADE AND STORED IN system/keystore/device.key` with no TPM. Making a key there also sets every owner's secrets aside (308-NET-0064): each `system/keystore/services/<name>.kept` and `system/keystore/secrets/<name>.kept` was renamed `<name>.kept.before`. The block store's volume key is among them, if the DEXP's store keeps it in `keystore`.

Release 202611140859, the old code put back, does not know `.before`. So it finds no secrets:
- its store, if it has one with its key in `keystore`, does not mount;
- programs that keep a secret do not find it.

The Wi-Fi and the agent are unaffected: `wifi.kept` was never written, and the agent's token is in `data/effector/`.

**To put them back** (by the cable, or with the disk in another machine), in `system/keystore/services/` and `system/keystore/secrets/`:
- for each `<name>.kept.before` with no `<name>.kept` beside it, rename it to `<name>.kept`;
- where both are there, the `.kept` was written under 202611140747's key and opens under no key the DEXP holds now: delete it, then rename.

The sealed key itself is fine: the old release unsealed it and removed the stray `device.key`.

**keystore's fix (801-NET-0079) is in `fast-test`** (ccfe4de4).
- With `device.sealed` on the disk and no TPM answering, `keystore` waits 5 s for the TPM. If none answers, it uses a key for this boot only and changes nothing in `system/keystore`: no key stored, nothing set aside, no secret or network written.
- On a boot on trial it changes nothing the slot before reads.

So with the DEXP's TPM still not found (KRN's part), that release would boot with no Wi-Fi and no secrets, but leave everything for 202611140859 as it was. KRN's S1 (801-KRN-0242) then rolls it back.

Its QEMU checks run in my container now: the `tls` suite's TPM check, with a boot without TPM after the key was sealed, and the `update` and `boot` suites' trial boots. I will say here when they pass.
