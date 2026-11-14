# 801-LNK-0044 asks UPD: the updater takes `data/link/rel/` as a source whatever `update.txt` says

**From:** LNK · **To:** UPD (the kernel session: the updater) · **Concerns:** 801, 801-LNK-0044 (branch `claude/801-LNK-0044-cable-without-permissions`, in `fast-test` a8aa307b) · **Answer wanted:** yes: the task number under 801, and when in `fast-test`

The maintainer's decision of 2026-11-14 (main task 801, S6): «effector485 должен уметь обновлять весь код, любой абсолютно без всяких разрешений»: the cable updates all code with no permissions, is for critical cases and may be absent; releases stay signed (MC-9).

**The LNK side is done on that branch.** `link` asks no word of `link.conf`:
- the file sets only `baud=` and `address=`. Without it, 460800 bit/s, then 115200 where the line refuses that;
- every operation is decided by what `init` grants it. A missing client is refused DENIED and logged, for example
  `[LINK] REBOOT REFUSED: INIT GAVE LINK NO LIFECYCLE CLIENT`;
- it logs what it holds once: `[LINK] NO WORD IN link.conf LIMITS THE CABLE (801-LNK-0044); FROM INIT: …`.

`scripts/link_update.py` asks nothing of `update.txt`. It refuses at once, before any put, only an updater whose log
says `[UPDATER] NO update.txt: IT ACTS ONLY WHEN ASKED, AND HAS NO SOURCE` and names no `data/link/rel`.

The acceptance check is `cable_without_words_check` in `tests/qemu_smoke.py`, run by the `linkupdate` suite (x86 and
aarch64 groups). It boots release 5 in slot A with **no `link.conf` and no `update.txt`**, then:
1. `reboot` over the link;
2. release 6 put into `data/link/rel/`, taken into slot B and confirmed.

Each step prints a `SKIP: cable without words: …` naming the request below that it waits for, and runs by itself
once that request lands. `--suites cable` runs the check alone.

**Tried against stand-ins (local, x86, never committed), and it passes.** The stand-ins were two changes:
- `init`: `boot_one`'s `&& self.link.is_none()` made `&& false`, and `link_dev()` made `true`;
- the updater: with no `update.txt`, `config()` returns `source data/link/rel`, `automatic apply`, `every 20`.

With them, `cable_without_words_check` passed on a disk with neither file: `reboot` over the link, then release 6
taken into slot B and confirmed. So the requests below are all the LNK side waits for.

No ABI or IDL change is needed for either request: `BootInfo.link_present` and `link_words` stay as they are, and
`link` reads only `baud=`, `address=` and (through `init`) `uart`.

## To UPD (the kernel session: the updater)

### 3. Take `data/link/rel/` as a source whatever `update.txt` says

- **What stops it today.** The updater's only source is the one `update.txt` names:
  - `updater/src/main.rs:108`: with no `update.txt` it logs
    `[UPDATER] NO update.txt: IT ACTS ONLY WHEN ASKED, AND HAS NO SOURCE` and keeps `source: None`,
    `automatic: None`;
  - `updater/src/main.rs:131`: the source is `update.txt`'s `source` line alone;
  - `updater/src/main.rs:1216-1218` (`automatic`): nothing runs without a source or with `automatic none`;
  - `updater/src/main.rs:611` (`with_source`): `check` and `fetch` answer `NoSource`.

  So a release put into `data/link/rel/` is taken only where `update.txt` says `source data/link/rel` with
  `automatic apply`. It is ignored with no `update.txt`, with another source (an `https://` server), or with
  `automatic none` or `check`.

  The shell cannot start it from the cable either: `update` is in neither `NOW` nor `ASKED` of
  `shell/src/clients.rs`, so a link line `update apply` is refused (`clients.rs:66`). That is APP's, and not needed
  if the updater acts on its own.
- **What to change.** `data/link/rel/`, the directory `link` writes as `rel/` and `scripts/link_update.py` fills, is a
  source of the updater's always, read as `Source::Directory("data/link/rel")` is today, with every check
  unchanged:
  - the key file taken as the next of the one trusted;
  - the channel's and the timestamp's signatures and expiry;
  - the manifest's boot-key signature, each file's size and SHA-256;
  - the version floor and the minimum.

  So the cable still installs only what the updater's keys signed (MC-9.2–9.4).
  - **Channel:** `stable`, what `scripts/link_update.py` publishes (`--channel` names another, for an updater that
    chooses otherwise). The timestamp's and recovery orders' replay records (`STAMP`, `ORDERS`, `kept_stamp` at
    `updater/src/main.rs:256`) are kept by channel name. If the network source's channel is also `stable`, the two
    sources share one record; that is UPD's to decide.
  - **When:** whenever `data/link/rel/timestamps/stable` is there and changed since the last look, every 20 s or so,
    and with `apply`, whatever `automatic` says for the other source. Never while a trial waits, as now.
    - A directory with no channel costs a stat and logs nothing. Today a `source data/link/rel` with nothing in it
      logs `CHECK: REFUSED: Missing` at each run (`fail`, `updater/src/main.rs:603`).
    - A newer release from either source wins. The cable's is refused `Older` like any other.
  - **The log:**
    - name the directory once at the start, for example
      `[UPDATER] SOURCE data/link/rel (THE CABLE'S, 801-LNK-0044): ALWAYS, AUTOMATIC APPLY`;
    - stop saying `HAS NO SOURCE` when there is no `update.txt`.

    `scripts/link_update.py` refuses an updater at once only when its log has that old line and no line naming
    `data/link/rel`.
- **`docs/update/updater.md`** and `idl/update.wit`'s `no-source` text ("update.txt names no source") change with
  it. No IDL version change is needed: only the documentation of when `no-source` comes.
- **`docs/profile/threat-model.md`**, the cable's entry: "The updater reads `data/link/rel/` only where `update.txt`
  names it." then becomes "The updater always reads `data/link/rel/` (801-UPD-…)."

### Acceptance (UPD)

- **QEMU, x86 and aarch64,** the `linkupdate` suite:
  - `cable_without_words_check` prints its `PASS: cable without words: …` line, once KRN's part is in too. On a disk
    with no `update.txt`, release 6 is put into `data/link/rel/` over the link, taken into slot B, booted on trial and
    confirmed. Before KRN's part the check stops at its first SKIP.
  - The existing `linkupdate_suite` (with `update.txt` naming `data/link/rel`) still passes. So do the `updater`
    suite's refusals (expired, changed signature, older).
- **Worth adding:** an `update.txt` naming an unreachable `https://` source still takes the cable's release.

## What remains after both

- **The bench:** the same on one machine (801-LNK-0044's second criterion). This needs a disk written from a tree
  with both changes, and the parent session's cable.
- **APP, optional:** `update status` and `update check` from the link at once, read-only. Then
  `scripts/link_update.py` could read the channel the updater follows instead of assuming `stable`.
