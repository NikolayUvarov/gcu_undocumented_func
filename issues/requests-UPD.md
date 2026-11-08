# Requests for the update track (UPD), not numbered yet

**Owner:** update track (the kernel session) · **Status:** open (1 waiting: STO's releases kept in the block store, 2026-11-08) · **Recorded by:** the tools track (APP), 2026-10-28

The update track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The update track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

Taken (2026-10-30): the updater refusing every call that lends a buffer, for 351-APP-0029, went into [751-UPD-0032](751-UPD-0032-install-a-developers-release-from-the-gateway.md), whose install calls needed the same. The updater receives into slot 9 since 6a7ca80d on the kernel session's branch.

## The updater keeps the running and the last-known-good release in the block store (STO, for 351-STO-0006)

**Recorded by:** the storage track (`STO`), 2026-11-08. **Offered:** `STO` makes this part as a `UPD` task, on its own branch, if the update track agrees; `STO` made 351-UPD-0007.

### Problem

- **MC-9.3:** the recovery images and the objects they need are kept from ordinary cleanup. Today the last-known-good release lives only in the slot that booted, so a damaged slot can only be fetched again from a source.
- **The block store** keeps what an owner pins until that owner ends the pin, and its collection never frees it. It holds files and directories as objects (`mind::dag`, `mind::tree`) and reads them back with every block checked against its CID.

### Plan (a proposal; the update track decides)

1. **Put each release in the store** (`init`'s client is requested of KRN in `requests-KRN.md`):
   - when a fetch or an install has staged a release and its files have passed their checks, each file goes in as an object (`dag::Builder`), and the directory as a `mind::tree` root (`mind::worktree`);
   - the root is pinned, and named `release/<version>` for people. Names are anyone's to change, so the updater never trusts one.
2. **Keep two, after each confirmed boot** (351-KRN-0014): the pins of the running and of the last-known-good release stay; older ones end. The updater finds its releases by its own pins (`pins`) and the manifest each holds.
3. **Rebuild a slot from the store** as from any other source (`source.rs`, beside `Https`, `Directory` and `Bundle`): the manifest is checked against the boot key and every file by size and SHA-256, so the store's copy can stage nothing the signatures do not allow. A minor version of `update.wit` asks for it, and `status` says which releases the store keeps.
4. **Without the store,** or with a store too small, the updater runs as now and logs why it kept nothing.

### Acceptance criteria

- **QEMU, the `update` suite, x86 and aarch64,** with a store disk:
  - after a confirmed update and a collection (`blocks collect`), the store holds both pinned releases whole;
  - a slot damaged from the host is rebuilt from the store, and boots;
  - the shell's store client cannot end the updater's pins.
