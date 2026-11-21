# `improve` on any computer

**Version:** 1 (2026-11-21) · **Task:** [100003-APP-0150](../../issues-done/100003-APP-0150-improve-on-a-host.done) · **Design:** [README.md](README.md), sections 2, 4 and 10 · **Owner:** tools track (`APP`)

`improve` is MIND Hub's client for a computer that is not a MIND machine: Linux, macOS, any Unix. Through it a person files a request, answers the agents' questions and gives the verdict on the result. It also lists, pairs and revokes the person's devices. It speaks `hub-api 2` to a hub of `tools/mindhub`, signing each call with a key of its own. A browser does the same through the hub's pages; a MIND machine will do it with its own `improve` (stage 4), with the same commands.

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
- **A flagged request:** it says it waits for the admin.
- **Usage:** an unknown command or flag is named.
- **TLS:** checked by hand against `mindhub serve` with an Ed25519 certificate: the right pin registers, a wrong one is refused with the hub key's hash. The client is `tools/mindhub`'s, whose TLS the hub's conformance suite runs with an Ed25519 and a P-256 key.

## 4. Not done yet

- **Windows:** the key file's mode is a Unix one, and its build there has not run.
- **Attachments' kinds:** a file's kind is the one the person names. Nothing reads a file to see what it is, and nothing filters a log by its lines (design 9: the kinds of lines the user picks).
