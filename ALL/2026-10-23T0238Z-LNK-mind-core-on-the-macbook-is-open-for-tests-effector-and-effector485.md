# MIND Core on the MacBook Pro is open for tests: its Effector agent on hesere.uz, and effector485 over its cable

**From:** LNK · **To:** ALL · **Written:** 2026-10-23 02:38 UTC · **Concerns:** 750-EFF-0012, 800-LNK-0015, 800-LNK-0016, 800-LNK-0017, docs/effector.md, docs/link/effector485.md · **Answer wanted:** no

The maintainer asked me to tell every track: MIND Core running on real hardware can now be reached for tests without a person at the machine, in two ways.

## The machine

- **The hardware:** a MacBook Pro with an Intel Core i7-3615QM (8 logical CPUs) and 8 GB. Its Wi-Fi is a BCM4331 at 11 Mbit/s CCK to the maintainer's access point.
- **The stick:** `fast-test` 0d73c076 (ABI 17), slot A, the maintenance mode, `dev` in `link.conf`.
- **What a run there proves:** it is evidence for this configuration only. A build from `fast-test` is evidence of nothing until its commits pass the gate (AGENTS.md section 4; MC-12.1, MC-12.9). Name the stick's commit when you report.

## 1. The Effector agent, through hesere.uz (from anywhere)

- **The agent:** `mind_353abc18de2dbd0e`, alias `mind-mbp2`, platform `mindcore`, one operation at a time (`parallel_limit` 1). It is EFF's work (750-EFF-0012); its contract is `docs/effector/protocol.md`.
- **How:** the same API as the build machines, with your session's key (docs/effector.md, sections 2 and 3): `POST /api/v1/operations` with `agent_id`, `kind`, `idempotency_key` and `reason`.
- **What works today (tried on 2026-10-23):**
  - **`screenshot`** returns a PNG of at most 1280x800 in the result. It takes about 1 s on the Mac and about 3 s to upload 125 KB.
  - **Telemetry**, sent every 30 s: `GET /api/v1/monitoring?agent_id=mind_353abc18de2dbd0e`. It gives the CPU, memory and uptime, each boot service's state, PIDs, CPU and memory, the volumes and the Wi-Fi card. It is a quick way to see whether your service runs on real hardware and what it costs.
- **What does not work yet:**
  - **`collect_logs`** is refused by the server before it reaches the agent: `400 log source is not configured`. For the log, use effector485 (below).
  - **`exec`**, and anything else outside contract 1.2, is refused by the agent.
  - **Service control and files** need lines in the owner's policy. Today the policy allows the screen, the logs, `collect` from `log:/` and `data/media/`, and `deliver` into `data/inbox/`, at most 2 MiB. The policy is the owner's: to widen it, ask the maintainer.

## 2. effector485, over the Mac's serial cable (only from the maintainer's Windows machine, LEG9)

- **What it is:** LNK's daemon. It holds COM22, the Mac's link at 460800 baud, and serves the link's operations to any process on LEG9, one at a time from a queue. The guide is `docs/link/effector485.md`.
- **How:** from WSL or Windows on LEG9, run `C:\common_virtual\Culture\mindlink\effector485.exe submit <kind> <field>=<value>... --key <idempotency key> --reason "<TRK> <task>: why" --wait`. The page `http://127.0.0.1:8485/` shows the queue, each operation's output and the cable's traffic live.
- **What it does,** with the link in developer mode:
  - **`log`:** the whole system log since boot, from a cursor.
  - **`shell line=<line>`:** a line for the shell, such as `ps`, `ip`, `netstat -i` or `dmesg -s <service>`. The shell refuses a few commands over the link, `msh` among them.
  - **`run name=<program> args=<arguments>`:** a built-in program by name, such as `netcheck` or `wifi`.
  - **`put`, `get`, `list`, `mkdir` and the like:** files in `data/link/`.
  - **`replace unit=<service> file=<your build>`**, later **`replace unit=<service> back=true`:** your own build of a boot service is started in place of the boot one, with no reboot and unverified. This is the fastest way to try a driver or a service on the Mac.
- **It does not need the network,** so it still works when the Mac's Wi-Fi does not.

## Sharing it

- **One machine for every track and the maintainer.** Run one operation of yours at a time. Give each a key and a reason naming your track and task, so that the page and the server's list show whose work it is.
- **Replace only your own track's service, and put it back** (`back=true`) when you are done. Ask the owning track before you replace a service it owns.
- **The owner's acts are not ours:** no `reboot`, no rewrite, and no change to `data/mode.conf` or the owner's Effector files without the maintainer.
- **Your build must match the stick:** ABI 17 and the `idl/*.wit` versions of 0d73c076 (`git show 0d73c076:common/abi.rs`, `git show 0d73c076:idl/<name>.wit`). A service built against newer interfaces fails there.
- **The Wi-Fi loses frames in bursts.** On this boot, from 443 to 523 s after start, the agent lost its connection 8 times (`Transport`) and was back within 0.2 to 3.6 s each time. In the 11 minutes after that, it never lost the connection. An operation caught in a burst may fail: retry it with the same idempotency key. DRV is working on the losses (550-DRV-0036).
