# effector485: a small Effector over MIND Link

Russian: [effector485_RU.md](effector485_RU.md).

`effector485` lets an agent hand a machine on the link's cable a batch of work and come back to it, as it does with the maintainer's Effector machines ([docs/effector.md](../effector.md), section 3). Task: [800-LNK-0011](../../issues/800-LNK-0011-effector485.md).

- **The daemon** holds the cable as its one master ([protocol](protocol.md)) and keeps a queue of operations.
- **The command line** submits operations and reads them.
- **The other end** is MIND Core's `link` or [`mindlink serve`](mindlink.md).

It is built with `mindlink` (`tools/mindlink/build.sh`): `effector485` for Linux and WSL, `effector485.exe` for Windows.

## Starting the daemon

```bash
effector485 daemon COM22 --baud 460800                    # Windows, the RS-485 converter
effector485 daemon /dev/ttyUSB0 --baud 460800             # Linux
```

| Option | Default |
|---|---|
| `--baud`, `--payload`, `--timeout`, `--tries` | as for `mindlink` |
| `--listen <host:port>` | `127.0.0.1:8485` |
| `--state <dir>` | `.effector485` in the home directory: the queue and the results, kept across restarts |
| `--key-file <file>` | none. With it, every call must carry the file's text in the `X-API-Key` header. An address other machines reach is refused without it |
| `--addresses <n>[,<n>...]` | `0`: one machine, without an address. Several machines on one cable (below) |

**Its first line** says where it listens. It then names each operation it starts, with its whole command (only the secret a shell line's `tpm seal` takes is left out), and says when the cable's other end comes or goes. While more than 512 bytes move in 5 s, it says every 5 s what the cable carried since it started, the speed each way, and how busy the line was: `the cable: sent 2.9 MiB (41.2 KiB/s), received 40.5 KiB (1.1 KiB/s); the line 92% busy` (800-LNK-0018). The line's share is reckoned at ten bits a byte at `--baud`.

**A restarted daemon:**
- finds its queue and its results, and keeps the idempotency keys;
- follows again the programs it had started;
- marks `lost` any other operation that was running when it stopped.

## Several machines on one cable

One RS-485 pair can carry several machines, each at its own address (800-LNK-0026, the [protocol's version 2](protocol.md)). The wiring and the addresses are in [mindlink.md](mindlink.md), "Several machines on one line".

```bash
effector485 daemon COM22 --baud 460800 --addresses 1,2,3
effector485 --agent link-2 submit log --key LNK-log-2 --reason "LNK: the second machine's log" --wait
```

- **Each address is a machine** of the API's: `agent_id` `link-<n>`, and `link` for 0, the one machine without an address a cable may also carry. Without `--addresses`, the daemon serves `link` alone, as before.
- **Each machine has its own:**
  - queue: its operations run one at a time, in order;
  - lease: a lease on `link-1` holds back `link-1`'s changes alone;
  - state: `online`, its name, when it last answered.
- **The line is shared, exchange by exchange.** Each machine has a worker of its own, and the workers take the line in turn for one request and its answer. A long `put` to one machine therefore slows the others but does not stop them. A machine that does not answer is asked once every 3 s, so it costs the others little.
- **Naming the machine.** With several, a submission and a lease name theirs, or are refused (400):
  - in the API, `agent_id` in the body;
  - on the command line, `--agent <machine>` or `EFFECTOR485_AGENT`.

  A lease's session and PIN hold only its own machine: an operation for another one with them is refused (409, `reservation_not_held`).
- **The state file** keeps each operation's `agent_id`. A restarted daemon that no longer serves a machine marks that machine's unfinished operations `lost`. A lease is kept in `reservation.json` for `link` and in `reservation-link-<n>.json` for the others.

## Operations

Each operation has:
- an `id`, which the daemon gives;
- an `idempotency_key` and a `reason`, both required.
  - **The key.** A second submission with a key already used answers the first operation and does nothing again. The same key for another kind is refused.
  - **The reason** names the track, the task and what runs, as on Effector.

**Statuses:**
- `queued`, then `running`;
- then one of `completed`, `failed`, `cancelled`, `timeout` or `lost`.

**The operations run one at a time,** in the order submitted: the cable has one master. A program started by `run` is followed between the other operations, so more work can be queued while it runs. A lease changes the order (below).

**Between operations,** after 2 s of quiet, the daemon sends the other end a POLL, which keeps the link's mark on its screen lit (800-LNK-0020) and finds a lost line before the next request does.

| Kind | Fields | Does |
|---|---|---|
| `hello` | — | the other end's name and protocol |
| `put` | `file` (a path on this machine), `name`, `force` | sends a file. `name` is the other end's (default: the file's own name), its directories made when missing; skipped when the other end holds the same bytes, unless `force` |
| `get` | `name`, `file` | fetches a file, checked by SHA-256 |
| `list` | `name` | a directory's listing; the top without `name` |
| `stat` | `name` | `size` and `sha256` in the result |
| `mkdir` | `name` | makes a directory and its parents |
| `move` | `from`, `to`, `replace` | moves or renames a file or a directory |
| `delete` | `name` | deletes a file |
| `rmdir` | `name`, `recursive` | removes a directory |
| `run` | `name`, `args`, `timeout_seconds` (600, at most 86400) | starts a program on the other end. Its output becomes the operation's; it completes when the program ends, with `exit_code` where the other end knows it (`null` otherwise); a fault fails it |
| `install` | `file`, `name` | puts a program into `apps/`, replacing one of the same name |
| `uninstall` | `name` | deletes `apps/<name>` |
| `log` | `from`, `max_bytes` | the other end's log from a cursor (default 0, 64 KiB); `next` in the result |
| `reboot` | — | restarts the other end (MIND Core: `dev` in `link.conf`); the daemon says hello again when it is back |
| `shell` | `line`, `timeout_seconds` | one line for the other end's shell (800-LNK-0015). Its output becomes the operation's; it fails when the shell refused or failed the line |
| `replace` | `unit`; `file` (put first, as `replace/<its name>`) or `name` (one the other end holds); or `back`: true | starts the other end's service again from that file, unverified, or from its boot image (800-LNK-0016; MIND Core: `dev` in `link.conf`) |

**Names on the other end** are paths below its directory: on MIND Core, `data/link/` (protocol, section 5).

**What the other end allows** decides the rest:
- MIND Core starts programs only with the word `run` in `link.conf`;
- with `dev`, the developer mode (800-LNK-0012), it also runs the built-in programs by name (`netcheck`, `wifi`), lends them the network, the Wi-Fi station and the system monitor that `init` grants it, and restarts on `reboot`;
- `mindlink serve` only with `--run`.

## The API

JSON over HTTP, as Effector's:

| Call | Answers |
|---|---|
| `POST /api/v1/operations` with `kind`, `idempotency_key`, `reason`, the kind's fields and, on a cable of several, `agent_id` | the operation: 201 when new, 200 when its key was used already |
| `GET /api/v1/operations?id=<id>` | the operation, the end of its output in `result.stdout` |
| `GET /api/v1/operations`, `?status=active`, `&agent_id=<machine>` | all, or those not ended, or one machine's, without their output |
| `GET /api/v1/operations/output?id=<id>&from=<n>` | the output from byte `n`: `text`, `from`, `next`, `status` |
| `POST /api/v1/operations/cancel` with `operation_id`, `idempotency_key`, `reason` | the operation. A queued one is cancelled at once; a running one when the daemon next reaches it |
| `GET /api/v1/endpoints` | each machine on the cable: `agent_id`, `address`, `online`, `name`, `last_seen`, `error`, `queued`, `running`; the cable's bytes since the daemon started, `sent_bytes` and `received_bytes`; its lease, `reservation` (`null` when there is none) |
| `GET`, `POST`, `PATCH`, `DELETE /api/v1/terminal-reservations` | the lease on the machine (below) |
| `GET /api/v1/api-key` | `valid`: the key is right |
| `GET /` | the page (below) |
| `GET /api/v1/stream` | the page's live feed, a WebSocket (below) |

**The output** keeps the last 256 KiB of each operation.

**Cancel** stops a put or a get between frames. It stops a running program with STOP, where the other end can: MIND Core's `link` does, through the loader (800-KRN-0138). Where it cannot, the cancel is recorded as `cancel_error` and the program goes on.

## The command line

```bash
effector485 submit put file=build/app.elf name=apps/app.elf --key LNK-app-1 --reason "LNK 800-LNK-0011: the app" --wait
effector485 submit run name=apps/app.elf args="--self-test" --key LNK-app-run-1 --reason "LNK: the app's self-test" --wait
effector485 submit log from=0 --key LNK-log-1 --reason "LNK: the log after the test" --wait
effector485 list --active
effector485 cancel e485-6ad7cdb8-7 --key LNK-cancel-1 --reason "LNK: wrong build"
```

- **Fields** are `name=value`. `true`, `false` and the numbers of `timeout_seconds`, `from` and `max_bytes` are taken as such.
- **`file`** is made an absolute path, since the daemon's directory is not the caller's.
- **`--wait`** prints the output as it comes and ends with the operation:
  - exit status 0 when it completed;
  - the program's exit code when it is known and not 0;
  - 1 otherwise.
- **`--at <host:port>`** or `EFFECTOR485_AT` names the daemon, and `--key-file` or `EFFECTOR485_KEY_FILE` its key.
- **`--agent <machine>`** or `EFFECTOR485_AGENT` names the machine on a cable of several, for `submit`, `reserve` and `list`.

## Reservations

Several agent sessions share one cable. A job of many steps, such as a slot update (some 100 `put`, then a trial restart), takes a **lease** on the machine, so that no other session's `reboot` or `replace` lands in the middle of it. Task: [800-LNK-0021](../../issues/800-LNK-0021-effector485-reservations.md).

- **One lease at a time on each machine.** It has:
  - `session_id`: `R-` and 8 characters, given by the daemon;
  - `pin`: 12 characters, answered once, when the lease is taken;
  - `owner`, free text such as a track's code, and `reason`, required; everyone sees both;
  - `created_at` and `expires_at` (Unix seconds), `remaining_seconds`, `duration_minutes`, `renewal_attempts`.
- **Its term** is 1 to 60 minutes. A renewal sets its end to now plus the term, as often as the work needs: the term is there so that a forgotten lease frees itself, not to limit the work. At its end it ends by itself, and the daemon's log says `the reservation R-… of <owner> ended: expired` (or `released`).
- **What waits.** While the lease lasts, another session's `reboot`, `run`, `shell`, `install`, `uninstall`, `replace`, `put`, `mkdir`, `move`, `delete` and `rmdir` stay queued, wherever they write. Each keeps its `queue_position` and shows why, in the API, the feed and the page: `waiting: {"code": "terminal_reserved", "message": "the machine is reserved by EFF (…) until 05:30:00 UTC (R-…)"}`.
- **What passes:** `hello`, `log`, `list`, `stat` and `get` from anyone, and every operation of the holder's. The daemon starts the first queued operation that may run, so these overtake the ones that wait. A program started before the lease is still followed to its end.
- **The holder's operations** carry the headers `X-Effector-Reservation-Session` and `X-Effector-Reservation-Pin`; the API shows the session as their `reservation_session_id`. A submission naming a lease that is not held (expired, released, or with a wrong PIN) gets 409 `reservation_not_held`, unless its idempotency key was used already: then it answers that operation.
- **It outlives a restart:** the daemon keeps it in `reservation.json` beside `operations.json`.

| Call | Answers |
|---|---|
| `GET /api/v1/terminal-reservations` | `{"reservations": [<the lease>]}`, empty when the machine is free; never the PIN |
| `POST` with `duration_minutes`, `reason`, `owner` | 201 `{"reservation": …, "pin": "…"}`; 409 `terminal_reserved` with the lease that is held; 400 for a bad term or no reason |
| `PATCH` with `reservation_session_id`, `reservation_pin`, `duration_minutes` (default: the lease's own term) | 200 `{"reservation": …}`; 409 `reservation_not_held` |
| `DELETE` with `reservation_session_id`, `reservation_pin` | 200 `{"status": "released", "reservation": …}`; 409 `reservation_not_held` |

`PATCH` and `DELETE` take the session and the PIN from the two headers as well.

```bash
export EFFECTOR485_RESERVATION=$(effector485 reserve --minutes 30 --owner EFF --reason "EFF 81-mindcore-04: slot B")
effector485 submit put file=rel/kernel name=rel/kernel --key EFF-put-1 --reason "EFF: slot B, the kernel" --wait
effector485 renew --minutes 30
effector485 submit reboot --key EFF-reboot-1 --reason "EFF: slot B, the trial restart" --wait
effector485 release
```

- **`reserve --minutes N --reason R [--owner O]`** prints `SESSION:PIN` alone on its first line, and the lease's end on stderr.
- **`submit … --reservation SESSION:PIN`**, or `EFFECTOR485_RESERVATION` without the flag, sends the two headers; `cancel` sends them too.
- **From WSL to `effector485.exe`,** pass the flag. WSL hands a variable to a Windows program only when `WSLENV` names it, so `EFFECTOR485_RESERVATION` alone does not arrive (or set `WSLENV=EFFECTOR485_RESERVATION`). Without it, your own operations wait behind your own lease.
- **`renew [SESSION:PIN] [--minutes N]`** and **`release [SESSION:PIN]`** take the lease from there when it is not given.
- **`reservations`** lists the lease: session, owner, reason, end, seconds left.

**How it differs from the Effector server's reservations** ([docs/effector.md](../effector.md), rule 3), on purpose (EFF's request, 2026-10-23):
- the server refuses other sessions' operations with 423; here they wait in the queue, saying for whom and until when;
- the server's lease has no reason; here the reason is a field of the lease, shown to everyone.

## The page in a browser

Open the daemon's address, `http://127.0.0.1:8485/`, in a browser on the same machine. Task: [800-LNK-0017](../../issues/800-LNK-0017-effector485-page.md).

- **What it shows, live:** the cable's other end (online, its name, when it last answered), the lease (who holds the machine, why, until when), the bytes sent ↑ and received ↓ since the daemon started with the speed each way over the last seconds and how busy the line is, the queue, the newest 200 operations with the reason each waiting one waits, and the output of the one clicked as it comes. The address keeps the operation followed (`#<id>`), so a reload or a link shows it again.
- **Its buttons:** `hello`, the log (new since the last read, or all of it), the shell's `ip`, `netstat -i` and `ps`, a shell line, a program with arguments, a directory's list, a service replaced or put back, and a reboot. Each is an operation like any other, with its own idempotency key and the reason typed on the page. Those that change the machine ask first.
- **The feed** is a WebSocket at `/api/v1/stream`. Its messages are JSON: `hello` (whether the key is needed), `traffic` (every second: `sent`, `received`, the daemon's clock `ms` and `baud`; read with no lock), `links` (at least every 2 s: the machines on the cable, each with its lease as `reservation`), `ops` (when the list changes, each with its `waiting`) and `output` (for the operation the page follows with `{"follow": "<id>", "from": <byte>}`).
- **The cable comes first.** A feed never waits for the daemon's state: when the queue's worker holds it, the feed skips that turn. The feed holds it only to copy a few fields and at most 48 KiB of output, and writes to the browser afterwards. A browser that stops reading for 2 s is dropped, and at most 8 pages are served at once.
- **The key.** With `--key-file`, the page asks for the key once. It keeps it for the tab (`sessionStorage`), sends it in `X-API-Key` with each call, and as the feed's first message.
- **Other sites are refused.** A call whose `Origin` is not the daemon's own address gets 403. So does a call on a loopback address whose `Host` is not a loopback name with the daemon's port. Another site open in the same browser therefore cannot start operations or read the feed.

## Tests

`tools/mindlink/tests/effector485.rs` drives a daemon against `mindlink serve --run` over a pseudo-terminal, through:
- a batch of put, mkdir, move, list, run with its output and exit code, and log;
- an idempotency key used twice, which runs the program once;
- a cancel that stops a running program;
- a restarted daemon that keeps its results and keys;
- the page, the feed following a program's output to its end, eight pages that never read while operations go on, a ninth page refused, the key asked by the feed, and calls from another site or host name refused;
- a lease: a second one refused, another session's `mkdir` waiting while a `log` passes and the holder's own run, wrong PINs refused, the lease kept across a restart and shown in the feed, and the waiting `mkdir` run once the lease is released. A unit test in `ops.rs` ends a lease at its time and reads `reservation.json` back.
- two machines on one cable (800-LNK-0026), `mindlink serve --address 1` and `--address 2` on a simulated shared line ([common/mod.rs](../../tools/mindlink/tests/common/mod.rs)): both online, a submission naming no machine refused, each `put` and `list` reaching its own machine alone, a lease on `link-1` holding back its `mkdir` while `link-2`'s runs, the lease refused for `link-2`, and the held `mkdir` run once it is released. A unit test in `ops.rs` keeps each machine's queue and lease apart, and marks `lost` the operations of a machine the daemon no longer serves.

`scripts/host_tests.sh` runs it. It says nothing about a real cable or a MIND Core machine (MC-12.1); [800-LNK-0011](../../issues/800-LNK-0011-effector485.md) records what has run on them.
