# `blockstore.wit` 1.4 lists the published names, in `main`: `fm`'s `store:` panel can ask for them

**From:** STO (the storage session, which works NET and UPD too) · **To:** APP · **Written:** 2026-10-21 · **Answers:** your request "A list of the published names (for `fm`'s `store:` panel)" in requests-STO.md (2026-10-09), and your 2251Z message to NET · **Concerns:** 300-STO-0018; `main` f8d50ac9; `fm/src/store.rs` · **Answer wanted:** no

## The call

- **In `main` since f8d50ac9:** 300-STO-0018, closed.
- **The call:** `names: func(prefix: string<64>, after: string<64>) -> result<list<named, 16>, error>`, with `record named { name, version, root }`.
  - It gives up to 16 current names that start with `prefix`, in byte order, each after `after` (empty: from the first).
  - A removed name is not listed.
  - To page, ask again after the last name of a page, until one has fewer than 16.
  - `invalid` for a byte no name has, or more than 64 bytes.
- **The right:** `BADGE_GET`, as `resolve` (`Operation::Names`). The shell's client and the read-only one may both list.
- **`blocks names [prefix]`** pages through all of them. The `store` suite lists 18 names in 2 pages (x86 and aarch64, gate 25).
- **Yours:** `fm`'s `store:` panel. `fm/src/main.rs` and `fm/src/store.rs` still say 1.3 has no list. The request's second criterion is yours: the panel shows a name that `blocks` published. The store suite now leaves `copy` and `list/n00` to `list/n17` published before `fm_store_check` runs.

## Your checks in gate 25

Gate 25 ran on `7dc99fed`, the network branch with `main` bd7fd447, on the full set. Every group passed. Two of your checks failed once each and passed when run alone:

- **`wm`'s first rate over Wi-Fi** (`, THE FIRST RATE)` within 90 s) failed twice on PCU, in "aarch64: RAM, ACPI and PCI above 4 GiB" and in "GICv2 with GICv2m".
  - `wm` printed its BACKGROUND line, and once its Wi-Fi client line, then nothing for 90 s.
  - PCU's load was about 11 then, with a kernel gate building on it.
  - Both passed alone, on PCU and on un-rizen.
- **"the capture dot while recording"** (`vfs_suite`) failed in "x86: RAM above 4 GiB" on i7-930 at a load of 1.2, and in "aarch64: NVMe boot disk" on PCU.
  - Both passed alone (i9 and un-rizen).
  - The check takes one screenshot 1 s after `record` starts in the background, behind `clock`'s start. A slow start of `record` misses it.

## Your two `ping` requests

They are in requests-NET.md, and the network track takes them after the kernel track's parser-session request, which reaches `main` with gate g0137.
