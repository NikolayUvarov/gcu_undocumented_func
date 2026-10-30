# Application packages over Effector: contract 1.5, draft 1 (not built)

**Status:** a draft for the Effector server project's review, 2026-10-30. Nothing here is built or claimed (MC-12.3). It
answers points 1 and 2 of the package contract (`ALL/2026-10-24T0904Z-EFF-server-accepts-package-draft-0-2-point-3.md`),
which native EFF took on 2026-10-24. The package itself, the signature and the key role are UPD's contract draft 0.2
([750-UPD-0019](../../issues/750-UPD-0019-signed-application-packages.md)). Its point 3 is accepted by the server. This
draft adds only the wire: the command, the download, and the result.
**Tasks:** [750-EFF-0008](../../issues/750-EFF-0008-application-packages.md) (MIND), the server's
`81-mindcore-02-package-metadata`.
**Version:** the agent's contract becomes 1.5 when this is built. 1.3 (health, the delivery receipt) and 1.4 (a
developer's release) are built ([protocol.md](protocol.md)).

## 1. The command

The server's `deploy` command for a mindcore endpoint keeps its action. Only `new_hostport` takes a versioned MIND
form; its decoded string stays within the poll bound of 512 bytes:

```text
mind-package-1:<operation>:<package_id>:<size>:<sha256>
```

| Field | Rule |
|---|---|
| `mind-package-1` | The form's version. Missing, or another word: refused. |
| `operation` | `install`, `deinstall` (uninstall) or `reinstall` (update). Nothing else. |
| `package_id` | The server's package ID: 1–128 of `A-Z a-z 0-9 . _ -`. |
| `size` | The outer ZIP's size in bytes, decimal, 1 to 67 108 864, and no more than the owner's policy `max-bytes`. |
| `sha256` | The outer ZIP's SHA-256, 64 lower-case hex digits. |

- **Five fields, separated by `:`, and nothing else.** An extra field, an empty one, or a field out of its range is
  refused. A future field is a new version (`mind-package-2`), with its own transition.
- **The host form `operation:package_id`**, which Windows and Linux agents keep, is refused on a mindcore endpoint
  (`unsupported`), as both sides refuse it today. Nothing changes for other platforms.
- **The command's `timeout_seconds`** is the run time for the download, the checks and the install. Its default for
  `deploy` is 600 s. An answer later than it follows contract 1.4's rule for late answers (750-EFF-0025).

### The archive's `manifest.json`

As point 3 accepted it:
- `manifest_version` is 1.
- The agent's entry is chosen by `os` and its heartbeat `arch`: `x64` maps to `mindcore/x86_64`, `arm64` to
  `mindcore/aarch64`.
- An entry has `when` and `package` and no `cmd`:

  ```json
  {"when":{"os":"mindcore","arch":"x64"},"package":"mindcore/x86_64"}
  ```

**What the server refuses when it queues, and the agent refuses again on reading:**
- two entries for the same `os`, `arch` and operation;
- an entry with `cmd`;
- an entry with a field besides `when` and `package`, or a `when` field besides `os` and `arch`;
- a `package` directory other than the one its `arch` maps to.

The agent reads only its own entry, and only for the operation the command names. Entries for other platforms are the
server's; the agent ignores them, so a mixed package stays valid.

## 2. The download

- **The route.** `GET /api/deployment/download/{package_id}`, with the agent's ID header, as the server binds the
  download to an assignment it delivered (its SEC-96).
- **The checks.** The agent streams the bytes into the gateway, as for `deliver_file`.
  - A `Content-Length` other than `size`, or more bytes than `size`, ends the download: `digest`.
  - The SHA-256 is checked at the end.
  - The archive is opened only after both match.
  - No other route serves packages, and no redirect is followed.
- **`deinstall` downloads the archive too.** Its `MANIFEST` names the application to remove, and its signature shows
  that the remover holds the package. Removing by a bare name is not offered.

## 3. The result

Every command gets two answers, in this order:
1. `POST /api/deployment/result`, the server's `DeploymentResult`;
2. the command's ACK, `POST /api/commands/ack`.

Both carry the same outcome.

**`DeploymentResult` fields:**
- `agent_id`, `package_id` and `operation` are the command's.
- `script` is `mind-package-1`. MIND runs no script, and the field names the form instead.
- `success` and `exit_code` are `true` and 0, or `false` and 1.
- `stdout` is one line, below.
- `stderr` is empty.
- `started_at` and `finished_at` are the agent's clock, in UTC.

**The line in `stdout`, and the ACK.** A success is `completed`; a refusal is `failed` with `error` set to its code.

| Outcome | `stdout` | ACK |
|---|---|---|
| installed or updated | `installed NAME VERSION ARCH FILES MANIFEST_SHA256` | `completed` |
| removed | `removed NAME VERSION` | `completed` |
| refused | `refused CODE: what was checked` | `failed`, `error` CODE |

**The codes:**

| Code | When |
|---|---|
| `unsupported` | The command's form or version is unknown, or the agent's build has no package support. |
| `denied` | The owner's policy has no `packages` permission, no trusted `package-key` for the name, or no `package-downgrade` for an older version. |
| `digest` | The archive's size or SHA-256 differs, or a file's differs from its `MANIFEST` line. |
| `format` | The ZIP or `manifest.json` breaks a bound or a rule above, or `MANIFEST` breaks UPD's grammar. |
| `signature` | `MANIFEST.SIG` does not verify with the trusted key. |
| `arch` | The package's `arch` is not this machine's. |
| `system` | The running release is older than the package's `system` line. |
| `older` | The version is lower than the installed one, or the same version with other `MANIFEST` bytes. |
| `space` | `data/apps/` has no room. The previous version stays. |
| `busy` | The application runs, and the policy does not let the agent stop it (`deinstall`, `reinstall`). |
| `io` | The disk failed. The previous version stays. |

- **A transport check is never reported as an installed application.** `installed` is sent only after the files are
  written under `data/apps/<name>/<version>/` and `current` has been switched in one rename.
- **What a failure leaves.** Any failure leaves the previous `current` as it was.

## 4. What the server is asked to confirm

1. The command's form, field order and bounds above, and that `deploy` keeps its action name.
2. The default `timeout_seconds` of 600 for a mindcore `deploy`.
3. `script: "mind-package-1"` in `DeploymentResult`, and the `stdout` lines and codes as above. Or the server's own
   field for a code, if it has one.
4. That `GET /api/deployment/download/{package_id}` is the only route, with the bytes it serves bound to the command's
   size and SHA-256 when the command is queued.
5. The refusals at queueing listed in section 1.

The joint test's input (point 4) waits for 750-EFF-0008's install and KRN's 750-KRN-0169, as UPD said on 2026-10-24.
