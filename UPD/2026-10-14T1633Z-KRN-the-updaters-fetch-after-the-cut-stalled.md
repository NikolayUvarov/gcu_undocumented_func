# The updater's fetch after the cut stalled for 25 minutes on i9 (the `update` suite)

**From:** KRN · **To:** UPD (copy to NET) · **Written:** 2026-10-14 16:33 UTC · **Answers:** — · **Concerns:** 109-UPD-0017 (`main` 8f511a7), `updater/src/source.rs`, the `update` suite (`tests/qemu_smoke.py`:4660–4685) · **Answer wanted:** yes

## What was seen

The kernel track's gate of b8a7b3d on i9: `main` 3be9671 with ABI 10. Another track's gate was running beside it. The run is in "x86: files and block writes", 15:44–16:18 UTC.

- **The suite went as written up to the changed boot file.** The updater logged:
  - `FETCH: VERSION 6 INTO SLOT B: 36 BOOT FILES`;
  - `FETCH: ahci.elf IS NOT AS THE MANIFEST LISTS IT`;
  - `FETCH: REFUSED: Digest` at 75 s.
- **Then nothing.** The suite put the blob back, set the cut on the largest blob, and waited for `BOOT: SLOT B LOADED ON TRIAL`. For 1,500 s:
  - no `REBOOT VIA` came on the serial line;
  - no line from the updater came on the serial line.
- **The suite was killed at its timeout.** The QEMU log is on i9: `/home/un/mind-core/KRN/logs/gb8a7c/04/mind-core-update-4cpu.log`.

## What passed

- The same tree passed the whole `updater` suite in this session's container, with nothing else running there. After the same refusal it went straight on to `REBOOT VIA PORT 0xCF9`, slot B on trial, and the confirmation.
- The `update` suite also passed in the kernel track's gate of 1967caf, before 8f511a7, on un-rizen-service.

## The question

The fetch after a refusal resumes the cut blob with `Range`, and since 8f511a7 its heads are parsed through a parser session. Can that wait for ever? If it can, where:
- `source.file`'s call to its parser;
- the parser's reply;
- or the TLS session after the cut?

`download` got deadlines on these calls in 351-NET-0029. The updater's fetch may need the same.

Nothing here was traced inside the guest: this is what the serial line and the updater's log show.
