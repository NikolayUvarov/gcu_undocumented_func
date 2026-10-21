# The Effector agent's policy

**Version:** 1 (2026-10-21) · **Task:** [750-EFF-0003](../../issues-done/750-EFF-0003-interface-and-policy.done) · **Code:** `effector_gw/src/policy.rs`, tested by `tests/effector_policy_host.rs` · **Constitution:** MC-3.8, MC-3.11, MC-7.5, MC-10.7

The policy says what the Effector server may make this machine do. The owner writes it; the server never can.

- **Where it is:** `data/effector/policy`, written with the shell's `effector policy` command ([750-APP-0093](../../issues/750-APP-0093-the-shells-effector-command.md)).
- **Who reads it:** the gateway `effector_gw`, when it starts and when the owner asks (`reload-policy` in [`idl/effector.wit`](../../idl/effector.wit)), never on the server's request.
- **What is not named is denied.** An empty policy, a missing one or one that does not parse allows nothing.

## Form

- At most 4096 bytes and 64 rules, UTF-8, lines at most 200 bytes, ended by LF or CRLF.
- `#` starts a comment line; blank lines are skipped. Words are separated by spaces or tabs.
- The first line that is not a comment is `effector-policy 1`.
- **The whole file or nothing.** A line the gateway does not understand refuses the whole policy, and the refusal names the line. An unknown key is never skipped: a key from a newer version could narrow what the owner meant.

## Rules

| Rule | Allows | Bounds |
|---|---|---|
| `agent on` \| `off` | Anything at all; `off` (the default) denies every request. The agent runs only in the development or maintenance mode anyway ([750-EFF-0013](../../issues/750-EFF-0013-development-and-maintenance-modes.md)); this key goes in the grammar's next version | once |
| `service <name> <action>[,<action>…]` | `start`, `stop` or `restart` of that boot service | 16 services; never `init`, `shell`, `effector`, `effector_gw` |
| `log ring` \| `log boot` | The system log (`ring`) or the last boot's log on `log:` (`boot`) leaving the machine | each once |
| `collect <root>` | Files under `<root>` leaving the machine | 8 roots; `data/…/` or `log:` (`log:/…/`); never under `data/effector/` or `data/apps/` |
| `deliver <root>` | Files arriving under `<root>` | once; under `data/`, not `data/` itself, never a protected tree |
| `packages on` \| `off` | Installing, updating and removing signed applications | once |
| `package-key <name>` \| `* <key>` | Trusting an Ed25519 public key (64 hex digits) for that application's packages, or for every name | 8 keys; `packages on` and a key are both needed |
| `update off` \| `check` \| `fetch` \| `apply` | The furthest step the updater may take; each includes the ones before | once |
| `reboot on` \| `off` | Restarting the machine | once |
| `screen on` \| `off` | Capturing the screen | once |
| `test <grant>[,<grant>…]` \| `test none` | Running `msh` test scripts in the test account (501) with at most those `requires:` grants | once; never `firmware`, `authority`, `tls`, `ssh` |
| `max-bytes <n>` | The largest file, log, screen or package to move, in bytes | once; 1 to 1073741824; default 16777216 |

Names of services and applications are lower-case letters, digits, `_` and `-`. Paths are written as the Effector server writes them: `data/…` and `log:/…`.

## Paths

A root ends with `/`, and a path is under it only if the root is a whole directory name in it: `collect data/reports/` allows `data/reports/a.txt` and `data/reports/x/b.txt`, not `data/reportsx/a.txt`.

- A path with an empty, `.` or `..` part, a `\`, a control character or a space is refused.
- `data/effector/` (the agent's files), `data/apps/` (installed applications) and `system/` (private directories) never leave the machine and are never written by a delivery, whatever the roots say.
- `collect data/` is allowed and allows all of `data/` except those trees.

## An example

```
effector-policy 1
# The fleet may restart the monitor, read the log and reports, and drop files into inbox/.
agent on
service sysmon restart
log ring
collect data/reports/
collect log:
deliver data/inbox/
update check
max-bytes 1048576
```

Packages, the screen, reboots and test scripts stay denied here, because they are not named.
