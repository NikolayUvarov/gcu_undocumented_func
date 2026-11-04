# `regdb`: the pinned wireless-regdb catalogue

A `no_std` crate with no dependency: the station (`wlan`) and the radios read the same catalogue from it (550-DRV-0049). `src/lib.rs` is the interface, `src/tables.rs` is generated.

`db.txt` is the complete unmodified upstream regulatory database from
[`389b9b702cf9018e9a9078ccc4fa8eaae2e054e3`](https://kernel.googlesource.com/pub/scm/linux/kernel/git/wens/wireless-regdb/+/389b9b702cf9018e9a9078ccc4fa8eaae2e054e3),
release `master-2026-09-03`, fetched from the official repository
`https://git.kernel.org/pub/scm/linux/kernel/git/wens/wireless-regdb.git`.

- `db.txt` SHA-256: `b71c815bd90c284131a27ab5af6fb4951b3580dc8af83345f31fcd34c5cd0581`.
- Upstream ISC `LICENSE` SHA-256: `678b0df753c86198fc496d1f1033429bbd57f101472132ee7eaaf9f5e0a7fae1`.
- The exact copyright/permission/warranty notice is retained in
  [LICENSES/ISC-wireless-regdb.txt](../LICENSES/ISC-wireless-regdb.txt).

Generate `src/tables.rs` with `python3 scripts/regdb.py` from the repository root.
`--check` compares the committed output. No build-time download or firmware
payload is involved. All non-comment lines are validated, including WMM and
rules outside the current driver's supported band. Every unknown syntax/flag,
duplicate country/rule, incomplete WMM group, ambiguous complete 2G20 match or
input hash change requires explicit review.

The data is an upstream regulatory snapshot, with upstream's warranty
disclaimer, rather than an independently established current legal opinion.
The recorded source object and SHA-256 identify the input; no independently
trusted PGP signature verification is claimed. See the
[consumer contract](../hwdocs/rtl8852be-regulatory.md).
