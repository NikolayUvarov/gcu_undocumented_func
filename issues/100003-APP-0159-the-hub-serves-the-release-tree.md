# 100003-APP-0159 — The hub serves the release tree: the public channels, ranges, a rate per source network

**Type:** tools (`tools/mindhub`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — for the public channels; the restricted channels wait for the updater's signed fetch (UPD, asked in [docs/hub/README.md](../docs/hub/README.md) 8.3), the delivery link for `releases/<version>/COMMIT` (UPD, 8.4) · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), stage 3 · **Roadmap:** track C (update) and G · **Constitution:** MC-9.6, MC-10.3, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-11-25, as the first part of 100003's stage 3, at the maintainer's word of 2026-11-18: «и сюда же сервер поддержки обновлений — по факту эта система обновления и операционной системы и модулей для неё».

## Problem

- **The design** ([docs/hub/README.md](../docs/hub/README.md), section 8): the hub serves the release tree exactly as `scripts/release.py publish` writes it, read-only, through `tools/httpd`'s library, so that the same name checks and ranges apply.
- **Today** the hub has no route for it. A MIND machine takes its releases from `httpd` alone, and the hub and the updates are two servers.

## Plan

- **Where:** under `/updates/`, which no route of the hub's uses. An updater's `source https://<hub>/updates` then fetches `channels/stable`, `releases/…`, `blobs/…`, `keys/…`, `timestamps/…` and `recovery/…` there, as from `httpd`.
- **What it serves:** the directory `--releases DIR` names, a tree `release.py publish` writes. Without it, `/updates/` answers 404 and says the hub serves no releases.
- **httpd's code, used and not copied:** `layout::resolve` for the names, `layout::open` for the file (no link on the way, the file checked), `range::asked` for a byte range, `date::http` for the dates. The hub writes the answer's head as `httpd` does: `ETag`, `Last-Modified`, `Cache-Control` (a blob, a manifest and a key's version never change; a channel, its timestamp, its recovery order and the current key are fetched fresh), `If-None-Match` answered 304, one range answered 206 or 416.
- **The body** is read from the file in pieces and written as it is read, outside the hub's lock, so a large blob holds no request of the API back and no memory beyond a piece.
- **Channels:**
  - the public ones are named by `--public NAME`, as many as given, `stable` alone when none is;
  - another channel's file (`channels/`, `timestamps/`, `recovery/`) is answered 404, naming why: restricted channels are served on the device's signature, which the updater does not give yet (8.3);
  - blobs, manifests and keys are the same for every channel and are served to anyone.
- **A rate per source network:** `--updates-per-minute N` requests a minute (600 by default, as `httpd`), counted for the same network as the API's buckets (`util::network`), answered 429 with `Retry-After` past it. The hub's limits on connections apply as to every request.
- **Only GET and HEAD,** with no body and no query: anything else is refused with its status, as `httpd` refuses it.
- **Logged** at the start: `MINDHUB: THE RELEASE TREE <dir>, CHANNELS stable OFFERED UNDER /updates/`.

## Acceptance criteria

- **Host tests** (`tools/mindhub/tests/updates.rs`, a hub on loopback over a tree written as `release.py` lays it out):
  - a channel, a manifest and a blob fetched, their bytes and headers;
  - HEAD with no body;
  - a range 206, a range past the end 416, `If-None-Match` 304;
  - a restricted channel, its timestamp and its recovery order 404 with the reason; a public one served;
  - a name outside the layout, a dot name, a query, a symbolic link: refused as `httpd` refuses them;
  - POST refused;
  - the rate: the request past it 429 with `Retry-After`;
  - a hub with no `--releases`: 404 that says so;
  - the API answers while a blob is being sent.
- **QEMU** (x86): MIND Core's updater, its source the hub's `/updates`, finds and stages a release the hub serves.
- **Docs:** [docs/hub/server.md](../docs/hub/server.md) (the flags, the route, what is tested), the design's section 8 and 11.

## Progress (2026-11-25)

- **Written** as planned:
  - `tools/mindhub/src/updates.rs`: the route, `httpd`'s `layout::resolve`, `layout::open`, `range::asked` and `date::http`, the head as `httpd` writes it, the body sent as the file is read;
  - `server.rs` answers a path under `/updates/` before the API, outside the lock; `http.rs` takes HEAD there alone;
  - `mindhub serve … --releases DIR [--public NAME]… [--updates-per-minute N]`, and its line at the start.
- **Not like `httpd`:** the rate counts requests for the network `util::network` names (an IPv6 /64 as one), not for the address; the hub has no access log of its own, so a refusal is not logged.
- **Tested** in this container, x86_64, 4 CPUs:
  - `tools/mindhub/tests/updates.rs`, its two tests as the acceptance criteria list them, with the hub's 35 other tests;
  - **the QEMU suite `hubupdate`** (`tests/qemu_smoke.py`, in `scripts/ci_local.sh`'s group with `hub`): an image with release 5 in slot A, its `update.txt` naming `https://10.0.2.2:PORT/updates` with the hub's pin and `automatic fetch`. MIND Core's updater fetched release 6 from `tools/mindhub` and staged it in slot B: `113 FILES, 16684339 BYTES FETCHED`. From the host the hub served stable's channel as published and refused the channel `test`, naming why. The run took about 90 s.
  - This is QEMU's x86 machine and the hub on the same host, not a hub across a network (MC-12.1).
- **Left:**
  - the restricted channels, the packages and the delivery link, which wait for UPD (Blocked by);
  - a rate in bytes.

## Related

[100003](100003-mind-hub-requests-conversation-check-and-updates.md), [110](110-our-own-web-server.md) (`httpd`, `NET`), [docs/update/publishing.md](../docs/update/publishing.md), [docs/update/updater.md](../docs/update/updater.md) (`source`).
