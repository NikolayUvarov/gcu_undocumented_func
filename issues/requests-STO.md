# Requests for the storage track (STO), not numbered yet

**Owner:** storage track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-08, for main task [351](351-self-update.md) at the maintainer's request

The storage track numbers its own tasks (`NNN-STO-MMMM`), so requests from other tracks wait here. The storage track turns each into a task and removes it from this file, and the file goes when it is empty.

## Releases as objects, with the last-known-good pinned (351, phase 4)

### Problem

Self-update stages a release in slot B of the boot volume (351-UPD-0006). MC-9.3 asks that recovery images and the objects they need be protected from ordinary cleanup, and the block store's pins (303-STO-0002) and a durable medium are the natural home for that. The block store lives on a RAM disk today.

### Plan (a proposal; the storage track decides)

- Once the block store has a durable medium, the updater also puts each release as a DAG object of its blobs.
- The running and last-known-good releases are pinned under recovery roots that collection never removes.
- A slot can be rebuilt from the store if the boot volume's copy is damaged.

### Acceptance criteria

After collection runs, the last-known-good release is still complete in the store, and a damaged slot is rebuilt from it.
