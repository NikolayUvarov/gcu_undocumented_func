# The separate hub, `tools/mindhub`

**Version:** 4 (2026-11-25): the release tree under `/updates/` (100003-APP-0159); 3 (2026-11-21): `hub-api 3`, the user's answer to a proposal (100003-APP-0155); 2 (2026-11-21): `hub-api 2`, export and deletion, attachments on messages, names on the board, the proof of work raised by itself, flags (100003-APP-0148); 1 (2026-11-21) · **Tasks:** [100003-APP-0146](../../issues-done/100003-APP-0146-the-separate-hub.done), [100003-APP-0148](../../issues-done/100003-APP-0148-the-hubs-remaining-parts.done) · **Design:** [README.md](README.md), sections 3 to 7 and 10 · **Owner:** tools track (`APP`)

`mindhub` is MIND Hub's first server, a host program for Linux. It takes users' requests and their conversation, registers devices with the admin's answer, keeps each user's trust and the anti-flood limits, shows the board, and serves the agents' API and the admin's pages, all over `hub-api 3`. The forms, bounds, states, trust bands, buckets and signatures are `improve/proto`'s, the same code MIND Core's programs link. TLS 1.3 is `tools/httpd`'s.

This page says what is built and tested on the host. It is not a guarantee about any other configuration (MC-12.1, 12.3).

## 1. Running it

```sh
cargo build --release --manifest-path tools/mindhub/Cargo.toml
mindhub key admin.key        # prints the public half; the file is 0600 and stays with the admin
mindhub key intake.key       # one key for each agent: intake, each track's, the integrator's
mkdir hub && cat > hub/hub.conf <<EOF
admin maintainer <admin public key>
agent APP intake <intake public key>
agent APP track <APP's agent public key>
agent KRN integrator <the integrator's public key>
setting global-cap 60
EOF
mindhub serve hub --listen 0.0.0.0:8443 --cert chain.pem --key key.pem
```

- **TLS 1.3 alone, off loopback.** Without `--cert` and `--key` the hub listens on a loopback address alone.
- **Its key's pin.** At start it prints the key's SPKI SHA-256 (`MINDHUB: TLS 1.3, THE KEY'S SPKI SHA-256 …`), which devices pin as the updater pins its server. A certificate renewed in place is loaded again within 10 seconds.
- **The admin pages,** `/hub/admin/`, open with a code: `mindhub admin-code --key admin.key --hub https://HOST:8443 --pin <SPKI SHA-256>`. The code works once, within 10 minutes. The admin's session lasts 12 hours.
- **A user's pages,** `/hub/`, open with a pairing code from one of their approved devices (`POST /hub/v1/devices/pair`, `improve pair` from stage 4). The session lasts 30 days and ends when that device is revoked.
- **The board** is `/hub/board`, and its JSON is `/hub/v1/board`.
- **The release tree** (design 8; 100003-APP-0159): `--releases DIR` serves a tree `scripts/release.py publish` wrote, under `/updates/`.
  - An updater's `source https://HOST:8443/updates` with the hub's pin fetches `channels/`, `releases/`, `blobs/`, `keys/`, `timestamps/` and `recovery/` there, as from `httpd`.
  - Only the public channels: `--public NAME`, as many as given, `stable` alone when none is. Another channel's file is answered 404 with the reason: a restricted channel waits for the updater to sign its fetch (design 8.3).
  - The names are checked and the file opened by `httpd`'s own `layout`, a byte range read by its `range`: what `httpd` refuses, the hub refuses alike. GET and HEAD alone, no query and no body.
  - A blob, a manifest and a key's version are kept for a year by a client; a channel, its timestamp, its recovery order and the current key are fetched fresh. `If-None-Match` is answered 304.
  - The file is sent as it is read, outside the hub's lock: a large blob holds no call of the API back.
  - `--updates-per-minute N` (600 by default) requests a minute from one network, answered 429 with `Retry-After` past it. The hub's limits on connections apply too.
  - At start: `MINDHUB: THE RELEASE TREE DIR, CHANNELS stable OFFERED UNDER /updates/`. Without `--releases`, `/updates/` answers 404 and says the hub serves no releases.

## 2. Its data: one directory

| File | What |
|---|---|
| `hub.conf` | The admins' and agents' public keys and the settings it starts with. A line out of form stops the start, naming it. |
| `journal.jsonl` | Every change, one event a line, synced before it is answered. The state is the journal's replay. |
| `attachments/` | Attachments' bytes, each under its SHA-256, written whole before the event that names them. |

- **The journal is the record (MC-10.3).** Each event names its time and author, and a state change its reason.
- **Each event is checked against the state before it is written.** So the journal never holds one that the state did not take.
- **A crash.** A last line cut by a crash was never answered: the next start drops it and says so. Any other line that is not an event, or one that does not fit the state (a move the state machine does not make, for one), stops the start, naming its line.
- **A journal that cannot be written** stops the hub.
- **Settings.** The admin changes them on the settings page. Those changes are events, and they win over `hub.conf`.
- **Deleting rewrites the journal** (design 9). A request deleted by its user, or an account deleted, takes its texts out of the journal. The hub writes the journal again without them, in a file of its own that then takes the journal's place, so a crash leaves the old or the new one whole. A start does it again if a crash came between.
- **`hub-api 3`'s journal.** A journal of `hub-api 1` or `hub-api 2` is refused at start with that name: the forms in its events are of a version before.

## 3. What it does

- **Registration** (design 4.2, 4.3): `pending` with the device's first challenge.
  - The admin approves, asks or rejects, each with a message the device reads in `devices/me`. Several rounds of questions and answers are allowed.
  - A pending device has one held request, and 5 messages a day to its registration.
  - Approval releases its held requests to intake. Rejection drops them, and the key is refused from then on, with the admin's reason.
- **Signatures** (4.1): every call but the challenge and the board is signed over a challenge the hub gave.
  - A challenge is good for one call and 10 minutes, and every answer carries the next one in `Hub-Challenge`.
  - One network holds at most 32 challenges, so a flood pushes out only its own.
- **Trust and anti-flood** (5): the bands' limits are token buckets per user. A change of band takes effect from its moment.
  - Per network: 5 new keys a day, 1 after a rejection within the day, and 3 held requests a day.
  - The global cap holds users below band 2.
  - Repeats are refused for 30 days with the earlier number. A client ID files a request once.
  - The proof of work is turned on by the admin, as `work-bits`, its floor. From 8 new keys in an hour it is a bit more, and a bit more for each doubling, up to 28. `GET /hub/v1/challenge` says what it is.
  - The hub moves trust by itself only within −256 … +512. A negative trust is read-only, −1023 a ban.
  - Every refusal names its limit, and a 429 says when it ends (`Retry-After`).
- **Threads and states** (6): questions move a request to `needs-info`, and the last answer takes it back.
  - A proposal of an agent waits for its user, who accepts it, corrects it or declines it once (`consent`, `hub-api 3`). Its answer moves no state. A request's summary says `open_proposal`, and the agent reads the answer's `consent` in the thread.
  - The agents' moves are checked by the crate's state machine and their track.
  - The admin's word that a release carries landed requests (`POST /hub/admin/v1/delivered`) makes them `delivered`. A device that runs that release or a later one makes them `awaiting-check` (`POST /hub/v1/devices/me/version`).
  - A verdict of `works` verifies a request, and the others reopen it. No verdict in 30 days closes it unchecked.
- **The agents** (7): intake reads what is in intake, a track its own requests, the integrator what a track took, and any agent a public request that is not about security. None reads a held one, a flagged one or a deleted one.
- **Flags** (6.2): a request whose words ask for a credential, a key, a token, root or admin access, in English or Russian, is flagged and waits for the admin. The admin clears it with a reason, or moderates it. It is a list of words: it flags what it names and misses what it does not.
- **Attachments:** a request's, and an answer's or a verdict's, within the band's count and size, each checked against its SHA-256. They are kept by SHA-256 and served as downloads, never shown.
- **The board** names the user of a request only when they allowed it (`named`), and none once they deleted their account.
- **A user's data** (9):
  - `GET /hub/v1/me/export`, or the page's export, gives all the hub keeps of them as JSON.
  - `DELETE /hub/v1/requests/{id}` deletes a request no track took.
  - `DELETE /hub/v1/me`, or the page's form, deletes the account: their requests that no track took, their devices revoked, their names, contact and answers erased. A registration's network stays a day, for the anti-flood count.
- **Pages** without scripts: the user's list, thread, answer, a proposal's Accept, Correct and Decline, verdict, new request, deletion, export and account deletion; the admin's devices, users and trust history, requests with moderation and flags, settings and journal.
  - Each form carries its session's token. Cookies are `Secure`, `HttpOnly` and `SameSite=Strict`.
  - The pages load nothing from elsewhere (`Content-Security-Policy`).

## 4. Its tests

`scripts/host_tests.sh` runs them (`cargo test --manifest-path tools/mindhub/Cargo.toml`).

- **`tests/hub.rs`:** the state and commands on a clock of the test's own, with the journal on disk.
  - Registration asked, answered and approved; a held request released; a rejected key refused.
  - The bands' limits; a negative trust and a ban; a flood from one network; the global cap; a repeat.
  - A thread with questions and answers; the agents' states through verified and through reopened; closed unchecked after 30 days.
  - What agents read; the board; the hub's own trust bounds; pairing and a browser's session ended by revoking; the proof of work; `intake approved-only`.
  - A restart rebuilding the same state, a cut last line dropped, a broken or forged line refused.
  - 100003-APP-0148:
    - names on the board by the user's leave;
    - an answer's attachments within the band;
    - a deleted request, and a deleted account, gone from the journal word by word, and the replay the same;
    - an export;
    - the proof of work raised and lowered by the hour;
    - flagged requests waiting for the admin;
    - a journal of `hub-api 1` or `hub-api 2` refused by its name.
  - 100003-APP-0155: a proposal refused an answer with no consent, accepted with no words while a question waits (which its answer still ends), answered once, and a second one corrected with no words refused and declined.
- **`tests/conformance.rs`:** the conformance suite (`src/conformance.rs`, 20 checks, with deletion, export, an answer's attachment, a name on the board, an account deleted, and a proposal corrected by its user) against the hub on loopback, and over TLS 1.3 with an Ed25519 and a P-256 key, the hub known by its pin. A wrong pin is refused, and a second hub on the same data serves the same.
- **`tests/pages.rs`:** a browser signed in by a pairing code sees a proposal waiting with its three answers; a correction with no words is refused, and an acceptance with none reaches the track.
- **`tests/updates.rs`** (100003-APP-0159): a tree laid out as `release.py` publishes it. A channel, a manifest, a blob, the key and a timestamp fetched with their bytes and how long each is kept; HEAD; a range, one past the end, a tag held; a restricted channel's three files refused with the reason; a name outside the layout, a dot name, a query, a symbolic link, a name that is not a digest, POST, and HEAD on the API refused; the API answering while 32 MiB go to a reader that does not read; the rate's 429 and `Retry-After` while the API still answers; a hub with no tree.
- **The QEMU suite `hubupdate`** (100003-APP-0159): MIND Core's updater, its `source` the hub's `/updates` with its pin, fetches release 6 from `tools/mindhub` and stages it in slot B; from the host the hub serves stable's channel as published and refuses a restricted one.
- **`tests/http.rs`:** heads out of form refused with their status, forms decoded strictly, networks, codes, versions, and the challenges of one network.

**The conformance suite on another hub:** `mindhub conformance --hub URL --pin … --admin-key … --intake-key … --track-key … --track TRK --integrator-key …`.
- It needs the hub's admin key and three agent keys.
- It needs a network that registered no key on that hub within a day: it registers three, and checks that a rejection holds back the fourth.
- MIND Core's QEMU checks (stage 4) and the native hub (stage 7) are to run it.

## 5. Not done yet

- **Updates** (design 8), the rest of stage 3:
  - the restricted channels by band, which wait for the updater to sign its fetch (UPD);
  - the modules' packages, when UPD's format names a signed index;
  - the delivery from the release itself, which waits for `releases/<version>/COMMIT` (UPD); until then the admin marks a release delivered;
  - a rate in bytes per user and per network: today the rate counts requests;
  - MIND Core's updater on a real machine against a hub across a network: in QEMU it fetched a release from the hub (the `hubupdate` suite).
- **The intake agent's rule in AGENTS.md** (7.3) waits for the maintainer's word (stage 8).
- **The clients:** `improve` on MIND Core (stage 4) and on a host (stage 5). Until then the hub is driven by the conformance suite, the pages and the API.
