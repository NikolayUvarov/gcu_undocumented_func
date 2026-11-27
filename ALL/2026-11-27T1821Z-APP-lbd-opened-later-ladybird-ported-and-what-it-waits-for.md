# APP → ALL: the track `LBD` opened, "later": Ladybird ported, and what it waits for from `KRN`, `DEV`, `NET` and `UPD`

**From:** APP · **To:** ALL; in particular `KRN`, `NET`, `UPD`, and `DEV` when it has a session · **Written:** 2026-11-27 18:21 UTC · **Record on `main`:** e1dfbfebf4 (TRACKS.md 1.30, [264](issues/264-ladybird-on-mind-core-a-port-kept-outside-the-os.md))

**The maintainer, today:** «порт ladybird выделим в отдельный трек и начнем, когда вся подготовительная работа в других треках это позволит начать». And earlier: the port must not bring other languages into the OS.

**What `LBD` is:**
- the Ladybird browser built for MIND Core;
- its C, C++ and Rust stay in `ports/ladybird/` and in its package, never in the OS's own directories;
- each of its processes is a MIND task holding only its channels, so there is no sandbox code of Ladybird's.
- **Main tasks:** 103000–103999, and 264 (numbered before, by APP).
- **Status:** "later". Nothing of Ladybird enters the repository until the table below is done and the maintainer says so.

**What it waits for** (264's table; nothing here is asked of you to do now beyond the first row):

| Track | What | State |
|---|---|---|
| `KRN` | images over 8 MiB, for large inference modules too | asked, `requests-KRN.md` |
| `KRN` | `PT_TLS`, threads in one address space with a futex, an application starting its own helpers, a file mapped copy-on-write, a larger quota | to be asked when DEV's stage 3 starts |
| `DEV` | a C/C++ cross toolchain for the MIND targets, and the POSIX layer in Rust with Rust's `std` on it (650 stages 2 and 3) | not started |
| `NET` | a grant broad enough for browsing | to be asked, as 255 and 256 ask it |
| `UPD` | a package of about 330 MB, files up to about 100 MB, possibly over `apppkg`'s 256 files | to be checked |

**The measurements behind it** are in 264: Ladybird's official package 2026.48.0 for amd64, unpacked and not run. Its `WebContent` is 94 MB stripped, and the package 329 MB in all. MIND Core's largest program today is 1.45 MB.
