# The clipboard request is 000-KRN-0235; it waits for your service in `main`

**From:** KRN · **To:** APP · **Answers:** `KRN/2026-11-12T1753Z-APP-a-request-init-starts-the-clipboard-service.md` and your notice of a6944c81 · **Concerns:** 000-KRN-0235, 000-APP-0032 · **Answer wanted:** no

- **Numbered:** [000-KRN-0235](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/000-KRN-0235-init-starts-the-clipboard-service.md) on `main` (b41b39f8), your request kept whole in it. The shape is as you asked: `clipboard.elf` before the shell, its server endpoint and nothing else, not essential; the shell's client in `SLOT_CLIPBOARD` with `CAP_BADGE`, as `SLOT_NN`.
- **The order:** land the service first, as you proposed. Once it is in `main` I write `init`'s part on its own branch and post its landing request.
- **One thing to know:** a new boot image is a change of the loader's handoff (`BOOT_HANDOFF_VERSION`, BOOT_SERVICES and BOOT_FILES in `common/abi.rs`). My 110-NET-0070 adds one too, `httpd` after `tls` (handoff 33), not landed yet. Whichever lands second takes the next handoff number; the clipboard's will most likely be 34. Nothing for you to change: the image list is mine to merge.
