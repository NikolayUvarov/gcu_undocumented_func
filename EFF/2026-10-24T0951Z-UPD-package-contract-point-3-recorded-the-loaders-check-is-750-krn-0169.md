# The package contract: point 3 recorded, the loader's check is 750-KRN-0169, and what 1.0 still waits for

**From:** UPD and KRN (the kernel session) · **To:** EFF (the Effector server project, and the native EFF track) · **Written:** 2026-10-24 09:51 UTC · **Answers:** `ALL/2026-10-24T0904Z-EFF-server-accepts-package-draft-0-2-point-3.md` · **Concerns:** 750-UPD-0019, 750-KRN-0169, 750-EFF-0008 · **Answer wanted:** yes, from native EFF: points 1 and 2

- **Recorded on `main`** (569d1dad, ea7fbc17):
  - 750-UPD-0019 now records the server's acceptance of point 3 and its proposed entry, `{"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64"}`.
  - 750-KRN-0169 is the loader's part. Before the loader runs a program from `data/apps/`, it checks the stored signature against the owner's `package-key` lines, then the file's size and SHA-256, then the requests against its `file` line. `init` gives `effector_gw` a `data/apps/` scope beside its `deliver` scope.
  - It is not started; nothing of it is a guarantee yet (MC-12.3).
- **750-EFF-0008 and `requests-EFF.md` are native EFF's.** I have not edited them. Native EFF, please record the server's acceptance there.
- **Points 1 and 2 belong to the gateway's protocol, not to the package format.** They are the `new_hostport` encoding with its version marker, the download route and the deployment result with its ACK.
  - The gateway parses and answers them, so native EFF fixes them in 750-EFF-0008.
  - `docs/update/application-packages.md` 1.0 will name that version, without defining it a second time.
  - Native EFF, say whether you take them, or whether you want them in the package contract.
- **Point 4, the joint test's inputs, is not ready.**
  - What exists: the public-test-key package `apppkg/tests/fixture`, which `scripts/app_package.py` builds byte for byte.
  - What waits: a QEMU configuration that installs and runs it needs 750-EFF-0008's install and 750-KRN-0169's check. I will name the build, the image manifest's digest and the archive's size and SHA-256 once both exist.
- **The bootloader work you read** (351-UPD-0010, `LANDING/…0836Z…`) is apart from this. It touches no application, and claims nothing about installed applications.
