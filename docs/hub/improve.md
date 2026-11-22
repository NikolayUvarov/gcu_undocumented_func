# `improve`: on any computer, and on MIND Core

**Version:** 5 (2026-11-21): `hub-api 3`, a proposal answered with `--accept`, `--correct` or `--decline` (100003-APP-0155); 4 (2026-11-21): the shell's notice and `wm`'s mark (100003-APP-0153); 3 (2026-11-21): the user's check of the result on MIND Core; 2 (2026-11-21): MIND Core's `improve` (section 5, 100003-APP-0151); 1 (2026-11-21) · **Tasks:** [100003-APP-0150](../../issues-done/100003-APP-0150-improve-on-a-host.done), [100003-APP-0151](../../issues/100003-APP-0151-improve-on-mind-core.md) · **Design:** [README.md](README.md), sections 2, 4 and 10 · **Owner:** tools track (`APP`)

`improve` is MIND Hub's client for a computer that is not a MIND machine: Linux, macOS, any Unix. Through it a person files a request, answers the agents' questions and gives the verdict on the result. It also lists, pairs and revokes the person's devices. It speaks `hub-api 3` to a hub of `tools/mindhub`, signing each call with a key of its own. A browser does the same through the hub's pages; a MIND machine will do it with its own `improve` (stage 4), with the same commands.

This page says what is built and tested on the host. It is not a guarantee about any other configuration (MC-12.1, 12.3).

## 1. Setting it up

```sh
cargo build --release --manifest-path improve/host/Cargo.toml
improve setup --hub https://hub.example.org:8443 --pin <the hub key's SPKI SHA-256>
improve register --display-name "Your name" [--contact you@example.org] [--name laptop]
improve status
```

- **Its directory:** `$XDG_CONFIG_HOME/mind-improve`, or `~/.config/mind-improve`; `--dir DIR` first names another.
  - `hub.conf` holds the hub's address and its pins.
  - `device.key` holds this device's Ed25519 key, made at `setup` and readable by its owner alone. The key never leaves the computer; the hub knows the device by its public half.
  - `seen` holds the time of the last news `watch` showed.
- **Only TLS off this machine:**
  - A hub off this machine is reached by `https://` alone, and known by its pin, as the hub prints it at its start. A certificate with another key is refused, whoever signed it.
  - `http://` is taken for a hub on this machine's loopback.
- **Registration:**
  - A new user gives a display name. The device waits until the admin approves it; `improve status` shows the admin's words, and `improve answer TEXT` answers a question.
  - Another device of the same user registers with `--pair CODE`, a code from `improve pair` on an approved device. It is approved by the code.
  - A proof of work, when the hub asks one, is solved before the registration is sent.

## 2. The commands

| Command | What it does |
|---|---|
| `new --title T --text T` (or `--text-file F`) | A request: `--kind fault` (the default), `improvement` or `capability`; `--private`; `--security`, private always; `--named`, the user's name on the board once it is a task; `--attach log:PATH` (or `hardware:`, `screenshot:`, `text:`) |
| `list`, `show N` | The user's requests, with a question that waits; one request's text, attachments and thread |
| `reply N TEXT` | A comment; with `--answer #Q` the answer to question Q, which may carry attachments |
| `reply N [TEXT] --answer P --accept`, `reply N TEXT --answer P --correct\|--decline` | The answer to the agents' proposal P: accepted, with or without words; corrected or declined, with them. `list` marks a proposal that waits, and `show` names the command |
| `check N works\|partly\|does-not-work --release R TEXT` | The verdict, once the device runs the release that carries the request |
| `watch` | News at the hub's interval: a request that changed, a question that waits. `--once` looks once. A hub out of reach is asked again later and later, up to 15 minutes |
| `devices`, `pair`, `revoke DEVICE` | The user's devices; a pairing code for 10 minutes; a device revoked, named by the start of its ID |
| `export [FILE]` | All the hub keeps of the user, as JSON |
| `delete N`, `delete-account --yes` | A request no track took, deleted; the account deleted (its requests no track took, its devices, its name) |

- **What leaves the computer is shown first.** `new` shows the request and each attachment: its kind, name, size and SHA-256, and the file it is read from. A reply with attachments is shown the same way. Then it asks `send it? [y/N]`, and anything but yes sends nothing. `--yes` answers for a script.
- **A refusal says why.** It names the hub's limit and its words, and, for a limit of time, when it ends: `refused (429 requests): 1 requests a day, a burst of 1, at this trust; again in 23 h 59 min`.

## 3. Its tests

`scripts/host_tests.sh` runs them: `cargo test --manifest-path improve/host/Cargo.toml`, against a hub of `tools/mindhub` on loopback in the same process.

- **Every command:**
  - the setup and the key's mode;
  - the registration with the admin's question and the answer;
  - a request with an attachment, refused by a no and sent by a yes;
  - band 0's limit, with when it ends;
  - a question and its answer with an attachment;
  - a verdict refused before the device runs the release;
  - `watch`;
  - a phone paired, listed and revoked;
  - the export;
  - a request and the account deleted, and their words gone from the hub's journal.
- **A proposal** (100003-APP-0155): marked in `list`, named in `watch`, its command in `show`; refused without `--answer`, with two consents, a correction with no words, and an answer with no consent; accepted with no words, a second one corrected, and the track reads both.
- **A flagged request:** it says it waits for the admin.
- **Usage:** an unknown command or flag is named.
- **TLS:** checked by hand against `mindhub serve` with an Ed25519 certificate: the right pin registers, a wrong one is refused with the hub key's hash. The client is `tools/mindhub`'s, whose TLS the hub's conformance suite runs with an Ed25519 and a P-256 key.

## 4. Not done yet

- **Windows:** the key file's mode is a Unix one, and its build there has not run.
- **Attachments' kinds:** a file's kind is the one the person names. Nothing reads a file to see what it is, and nothing filters a log by its lines (design 9: the kinds of lines the user picks).

## 5. On MIND Core

`improve` is a console program of MIND Core with the same commands (100003-APP-0151). Their words are given on its command line, and a quoted word, `"…"` or `'…'`, stays whole.

```text
improve setup mind.example.org 203.0.113.5 8443 <the hub key's SPKI SHA-256>
improve register "Your name" --name laptop
improve new fault 'The fan is loud' 'At idle, since the last update'
improve list
improve show 1
improve reply 1 'Two fans, one stopped' --answer 2
improve reply 1 --answer 4 --accept
```

- **Its authority** is the design's (section 9 of [README.md](README.md)):
  - **The network:** a flow grant to the hub's host on its port alone. A host its grant does not name is asked of the user (socket.wit 2.8), and a line of the network policy names it for good: `improve 203.0.113.5 tcp 8443 …`.
  - **TLS:** to the hub's pinned key alone (`connect-pins`, the SPKI SHA-256 the hub printed at its start), with no device certificate. Another key is refused, whoever signed it.
  - **Its key:** its own Ed25519 key, made at `setup`, kept as its own secret in `keystore` (`hub-key`). No other program reads it.
  - **Its files:** its own directory, `system/own/improve/`, for `hub.conf`. `setup NAME ADDRESS|dns PORT PIN [PIN]` writes it; `dns` resolves the name.
- **What it reads:** it holds a long-term key and the network, so it parses nothing of the hub's itself (MC-11.11).
  - The `parse` service reads each answer, and `improve/proto` builds its tree and checks it against the API's forms.
  - Each call is signed over a fresh challenge, read from the body of `GET /hub/v1/challenge`, as `mind::http` hands a program no header of the hub's own.
- **What leaves the machine is shown first:** `new` prints the request and asks `send it? [y/N]`, and `--yes` answers for a script.
- **Attachments:** `--attach log|hardware|screenshot|text:FILE` on `new`, on an answer and on a verdict.
  - The file comes from the user's files the shell lends (`REQUEST_FILES`, `ram:` and `data/` among them).
  - Each one is shown before it is sent, with its kind, name, size and SHA-256, and its bytes go after its request or message.
- **`watch [--once]`:** the news since the last look (kept as `seen` in its own directory), at the hub's interval, until Esc.
- **`export FILE`:** all the hub keeps of the user, written as the hub sent it into a user's file such as `ram:hub.json`. Nothing of it is parsed on the machine, so its length is the answer's bound, 64 KiB.
- **A proposal's answer:** `reply N --answer P --accept`, or with a text `--correct` and `--decline`, as on a host.
- **What waits for the user** (100003-APP-0153): a request with an open question or proposal of the agents, or whose check waits.
  - `list`, `watch` and each change of the user's (`reply`, `check`, `version`, `delete`) keep their numbers in the user's files, `data/improve/waiting`, when they changed: a comment line, `change N` (the count of its writes), then the numbers.
  - The shell says them above its first prompt, as `improve` last saw them: `IMPROVE: 2 REQUESTS WAIT FOR YOU: #1 #4 (improve list)`. It reads the file every 5 s, a program running or not, and says a change above its prompt on the console shown once no program is in front there. `change` tells it of a write it missed: the same requests gone and back between two looks.
  - `wm` marks them at its top bar's right end, ` HUB 2 `, and a click names them.
  - The file is as fresh as `improve`'s last look: `improve watch &` keeps it so while it runs.
- **The version:** `improve version RELEASE` tells the hub the release this machine runs, as `update status` names it, so that a request a release carries awaits the user's check.
- **The check of the result** (design 6.3):
  - `show N` of a delivered request says whether the release that carries it is on this machine. If it is not: `your machine does not have release 202611211200 yet (it runs 202611201200): update apply, or wait for the update`. If the hub does not know what the machine runs, it says to tell it with `improve version`.
  - Once the machine runs it, the request waits for the check. `list` marks it, `watch` names it, and `show N` names the command.
  - `improve check N works|partly|does-not-work "WHAT YOU SAW"` gives the verdict on the release this machine runs, as the hub knows it; `--release R` names another.
- **Its check:** the QEMU suite `hub` (in the x86 network group) runs it against `tools/mindhub` on the host, over TLS by the hub's pin.
  - It goes from setup and registration through the admin's approval to a request with a log attached, the agents' question and its answer with a file, `watch`, the export, and a wrong pin.
  - The track's proposal waits, an answer with no consent is refused, and its correction from the machine reaches the track.
  - What waits: an earlier boot's file said above the first prompt; the agents' question said above a later prompt once `list` saw it, and again when the check waited; `wm`'s mark read off its screen.
  - Then the agents accept, work and land the request, and the admin delivers it in a release. `improve` says the machine does not run it, at an unknown version and at an older one. At that release the check waits, and the user's `works` verifies the request, with that release in the verdict.
  - The admin and intake read the attached bytes back. `tests/improve_host.rs` tests its words.
- **Not yet:**
  - **Long texts.** A thread with a text past 512 bytes, or an answer past 16 KiB, is named as unread: `parse` reads no longer string or document yet (`issues/requests-NET.md`).
  - **The running release:** not taken from the updater by itself. That needs a client of the updater that may only read its state, which `improve` does not get yet ([requests-UPD.md](../../issues/requests-UPD.md#a-client-of-the-updater-that-may-only-read-its-state-for-100003-app-0151)); until then the user tells it with `improve version`.
  - **Notices as they come:** the shell and `wm` say what `improve` saw at its last look. Nothing looks by itself unless `improve watch` runs.

