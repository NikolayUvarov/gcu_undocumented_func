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

**Its first line** says where it listens. It then says each operation it starts, and when the cable's other end comes or goes.

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
| `GET /api/v1/endpoints` | the cable's other end: `online`, `name`, `last_seen`, `error`, `queued`, `running` |
| `GET /api/v1/api-key` | `valid`: the key is right |

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

## Tests

`tools/mindlink/tests/effector485.rs` drives a daemon against `mindlink serve --run` over a pseudo-terminal, through:
- a batch of put, mkdir, move, list, run with its output and exit code, and log;
- an idempotency key used twice, which runs the program once;
- a cancel that stops a running program;
- a restarted daemon that keeps its results and keys.

`scripts/host_tests.sh` runs it. It says nothing about a real cable or a MIND Core machine (MC-12.1); [800-LNK-0011](../../issues/800-LNK-0011-effector485.md) records what has run on them.
