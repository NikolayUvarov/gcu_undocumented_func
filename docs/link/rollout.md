# A rollout: one release, one machine at a time

Russian: [rollout_RU.md](rollout_RU.md).

`scripts/rollout.py` gives one release to several machines in turn. It touches the next machine only once this one's Effector agent is back on the server running the release. Task: [801-LNK-0045](../../issues/801-LNK-0045-a-rollout-one-machine-at-a-time-and-the-cable-on-all.md), for 801's S8.

## Running it

```bash
# A dry run: the release is made locally; the server and the cable are only read.
TMPDIR=/mnt/c/common_virtual/Culture/mindlink/tmp-upd \
scripts/rollout.py --root usb_root mind_353abc18de2dbd0e mind_09dca8b4def0f6e6 mind_bc5d2f3b851df7a6 \
    --cable mind_353abc18de2dbd0e --mindlink ADAPTER --dry-run

# The rollout itself: the same line without --dry-run.
```

**The release:**
- `--root DIR`, a signed build, is made release `--version` (by default the time as `YYYYMMDDHHMM`), as `link_update.py` makes one. Its bundle is then written as `release.py bundle` writes one.
- `--bundle FILE` is a bundle already made. Its channel names the version, and it is checked as `release.py check` checks a server tree.

The same release goes to every machine.

**The machines** are Effector agent IDs, in order. Before anything is sent, the script refuses a machine named twice, one the server does not know, and one whose processor is not the release's.

**The server** is `--server` (`https://hesere.uz/effector`).
- The operator's API key is read from `--key-file` (`~/.config/mind-effector/hesere/api_key`).
- It goes in the `Authorization` header and nowhere else.
- It goes only over HTTPS, or to the machine's own loopback, and never follows a redirect.
- Every line the script prints is checked for the key.

**The cable** is used only for the machine `--cable` names:
- `--port` (`COM22`) and `--mindlink` go to `link_update.py`, as in [mindlink.md](mindlink.md).
- While `effector485` holds the port, `--mindlink` is an adapter that turns `hello`, `mkdir`, `put` and `log` into `effector485 submit … --wait` (the bench's own; none is in the repository yet).
- The adapter holds the lease, if one is wanted (`E485_LEASE_FILE`).
- `TMPDIR` must be a folder the Windows daemon can read, since the release's files are put from there.

**The bound** is `--wait` (1200 s), from the release's sending to the agent's return. `--poll` (10 s) sets how often the server is asked.

**`--run-id`** makes the operations' idempotency keys (by default the start time). Run again with the same ID, the script finds the operations it had already started.

## For each machine

1. **Through the server**, when the endpoint is online and taking operations (`online` and `operations_ready` in `GET /api/v1/endpoints?id=…`):
   - the bundle is uploaded once, `POST /api/v1/files`;
   - then `POST /api/v1/operations` sends `kind` `update_agent`, `artifact_id`, `version`, `timeout_seconds` and `queue_ttl_seconds` (the bound, at most 3600), an `idempotency_key` and a `reason`;
   - the operation is then read until it ends.
2. **Over the cable**, when the server cannot reach the machine, or its agent did not take `update_agent`. "Did not take" means an answer that came before a byte reached the updater: `denied` (`remote-update` off, or no `update apply` in the owner's policy), `no-policy`, `expired`, or a refusal by the server. The cable is used only for the machine `--cable` names, and only after its log over the cable names that agent:
   - the last `EFFECTOR: AGENT <id> TO …` line, or `NEW AGENT ID <id>`;
   - if the log names another agent, or none (the ring may have dropped the agent's start line), nothing is sent.

   `link_update.py` then puts the release and follows the updater's lines until `SLOT S CONFIRMED ON THE DISK`, or a refusal or rollback.

### Back

A machine is back when all of these hold:
- **Its answer.** Through the server, `update_agent` answered `completed`, `updated N, slot S confirmed`. Over the cable, the updater confirmed the slot.
- **The server.** The endpoint is online and taking operations again.
- **Its ring.** The script then reads this boot's log, `collect_logs` of `ring`, which is this boot's by definition. A saved `boot` log may still be the boot before's in a boot's first minutes. In the ring:
  - `[UPDATER] SLOT S[ ON TRIAL], VERSION N` must name release N;
  - with `--commit`, `[INIT] BUILD: BRANCH …, COMMIT …` must name that commit.

  The lines are printed as evidence.

**A late answer.** The server keeps only the status of a late answer (`late_acknowledgement`), not its text. Then the ring's version line is what proves the release.

### It stops the rollout

The script prints the machine, the reason and its log lines: this boot's ring and the newest saved boot log, or the cable's ring. The machines after it are not touched. It stops when:
- **The machine fell back:** `failed`, `fell-back`, `fell back: slot S runs version V, not N; the updater: …`. After 801's S1 and S2, this boot's updater logs `[UPDATER] A TRIAL REFUSED BEFORE (system/updater/refused): …` with the step that failed.
- **The release was refused or not confirmed:** `digest`, `signature`, `older`, `version`, `not-confirmed`, `restart`, `io`, and the rest. These are failures past the updater's door.
- **The machine did not come back within the bound.**
- **Its ring names another version or commit.**
- **Over the cable:** the updater's `REFUSED` or `ROLLBACK` line, or no confirmation within the bound.
- **No path:** the server cannot reach the machine, and it is not the one on the cable.

Exit status: 0 when every machine came back, 1 when the rollout stopped, 2 for a usage error or a release that does not verify.

## Tests

`tests/rollout_test.py`, run by `scripts/host_tests.sh`, drives the script against a stand-in server on `127.0.0.1`. The stand-in serves the routes and fields above, as hesere answers them. The cases:
- three machines updated in order, each sent the release only after the one before had answered, one of them by a late answer proved by its ring;
- a fallback stops the rollout before the third machine, with its reason and the updater's lines;
- a machine that never comes back stops it at the bound;
- a machine the server cannot reach, not on the cable, stops it;
- a dry run makes only GET requests and reads only the cable's log;
- an unknown machine or a wrong key sends nothing;
- the key is never printed, and is refused over plain HTTP to another host;
- a machine whose agent answered `denied` goes over a scripted cable once its log names it, and is back;
- a cable reaching another machine gets nothing.

This is host evidence only: no real server or machine was used (MC-12.1). The bench run is the issue's.
