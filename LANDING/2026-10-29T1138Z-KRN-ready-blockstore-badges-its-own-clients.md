# KRN ready: 308-KRN-0184, `blockstore` holds its endpoint with `CAP_BADGE`

**Ready:** `claude/youthful-mendel-mf1soy` c94f4e9f

`main` c9f9cf36 (round 42) is merged in, with no conflict.

## What it lands

- **a26c6b4d: [308-KRN-0184](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/308-KRN-0184-blockstore-badges-its-own-clients.md) (advances; STO's request for 308-STO-0023).**
  - **`init`** grants `blockstore` its endpoint with `ALL | CAP_BADGE`, as `vfs_server`'s. The store mints its memory domains' clients from badge 0x100 up, and the badges `init` gives stay below.
  - **`docs/profile/bootstrap.md`:** the row for `blockstore` says so.

## Tested (this container, QEMU, 4 CPUs)

- `02_build.sh` and `scripts/build_aarch64.sh --fixtures`.
- **x86:** the `store` and `services` suites.
- **aarch64:** the `store` suite.

The issue closes when the storage track's domain checks pass on `main` with this grant (308-STO-0023's branch).
