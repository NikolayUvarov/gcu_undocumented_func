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

**Its first line** says where it listens. It then names each operation it starts, with its whole command (only the secret a shell line's `tpm seal` takes is left out), and says when the cable's other end comes or goes. While bytes move, it says every 5 s what the cable carried since it started, the speed each way, and how busy the line was: `the cable: sent 2.9 MiB (41.2 KiB/s), received 40.5 KiB (1.1 KiB/s); the line 92% busy` (800-LNK-0018). The line's share is reckoned at ten bits a byte at `--baud`.

**A restarted daemon:**
- finds its queue and its results, and keeps the idempotency keys;
- follows again the programs it had started;
- marks `lost` any other operation that was running when it stopped.

## Operations

Each operation has:
- an `id`, which the daemon gives;
- an `idempotency_key` and a `reason`, both required.
  - **The key.** A second submission with a key already used answers the first operation and does nothing again. The same key for another kind is refused.
  - **The reason** names the track, the task and what runs, as on Effector.

**Statuses:**
- `queued`, then `running`;
- then one of `completed`, `failed`, `cancelled`, `timeout` or `lost`.

**The operations run one at a time,** in the order submitted: the cable has one master. A program started by `run` is followed between the other operations, so more work can be queued while it runs.

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
| `POST /api/v1/operations` with `kind`, `idempotency_key`, `reason` and the kind's fields | the operation: 201 when new, 200 when its key was used already |
| `GET /api/v1/operations?id=<id>` | the operation, the end of its output in `result.stdout` |
| `GET /api/v1/operations`, `?status=active` | all, or those not ended, without their output |
| `GET /api/v1/operations/output?id=<id>&from=<n>` | the output from byte `n`: `text`, `from`, `next`, `status` |
| `POST /api/v1/operations/cancel` with `operation_id`, `idempotency_key`, `reason` | the operation. A queued one is cancelled at once; a running one when the daemon next reaches it |
| `GET /api/v1/endpoints` | the cable's other end: `online`, `name`, `last_seen`, `error`, `queued`, `running`; the bytes since the daemon started, `sent_bytes` and `received_bytes` |
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

## The page in a browser

Open the daemon's address, `http://127.0.0.1:8485/`, in a browser on the same machine. Task: [800-LNK-0017](../../issues/800-LNK-0017-effector485-page.md).

- **What it shows, live:** the cable's other end (online, its name, when it last answered), the bytes sent ↑ and received ↓ since the daemon started with the speed each way over the last seconds and how busy the line is, the queue, the newest 200 operations, and the output of the one clicked as it comes. The address keeps the operation followed (`#<id>`), so a reload or a link shows it again.
- **Its buttons:** `hello`, the log (new since the last read, or all of it), the shell's `ip`, `netstat -i` and `ps`, a shell line, a program with arguments, a directory's list, a service replaced or put back, and a reboot. Each is an operation like any other, with its own idempotency key and the reason typed on the page. Those that change the machine ask first.
- **The feed** is a WebSocket at `/api/v1/stream`. Its messages are JSON: `hello` (whether the key is needed), `traffic` (every second: `sent`, `received`, the daemon's clock `ms` and `baud`; read with no lock), `link` (at least every 2 s), `ops` (when the list changes) and `output` (for the operation the page follows with `{"follow": "<id>", "from": <byte>}`).
- **The cable comes first.** A feed never waits for the daemon's state: when the queue's worker holds it, the feed skips that turn. The feed holds it only to copy a few fields and at most 48 KiB of output, and writes to the browser afterwards. A browser that stops reading for 2 s is dropped, and at most 8 pages are served at once.
- **The key.** With `--key-file`, the page asks for the key once. It keeps it for the tab (`sessionStorage`), sends it in `X-API-Key` with each call, and as the feed's first message.
- **Other sites are refused.** A call whose `Origin` is not the daemon's own address gets 403. So does a call on a loopback address whose `Host` is not a loopback name with the daemon's port. Another site open in the same browser therefore cannot start operations or read the feed.

## Tests

`tools/mindlink/tests/effector485.rs` drives a daemon against `mindlink serve --run` over a pseudo-terminal, through:
- a batch of put, mkdir, move, list, run with its output and exit code, and log;
- an idempotency key used twice, which runs the program once;
- a cancel that stops a running program;
- a restarted daemon that keeps its results and keys;
- the page, the feed following a program's output to its end, eight pages that never read while operations go on, a ninth page refused, the key asked by the feed, and calls from another site or host name refused.

`scripts/host_tests.sh` runs it. It says nothing about a real cable or a MIND Core machine (MC-12.1); [800-LNK-0011](../../issues/800-LNK-0011-effector485.md) records what has run on them.
