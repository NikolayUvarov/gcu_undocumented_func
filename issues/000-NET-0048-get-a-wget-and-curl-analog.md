# 000-NET-0048 — `get`, an analog of `wget` and `curl`

**Type:** network (a new program `get`) · **Owner:** `NET` · **Priority:** P2 · **Status:** open · **Blocked by:** [000-NET-0047](../issues-done/000-NET-0047-http-for-api-clients.done), [000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md) (a host named at run time), [000-NET-0046](000-NET-0046-public-root-certificates.md) (public sites) · **Main task:** — · **Constitution:** MC-3.1, MC-11.6, MC-11.11, MC-12.1

Recorded by the network track on 2026-10-25, for the plan [docs/communications-and-agents.md](../docs/communications-and-agents.md), at the maintainer's request: "an analog of wget/curl".

## Problem

- **The shell's `fetch` and `https`** take one URL and print or save what comes.
- **Nothing sends a POST, sets a header, uploads a file, follows a redirect, or tests an API** from the console or a script.

## Plan

**The program `get`** (its name is the maintainer's to change), a process of its own:

- **What it does:**
  - `get URL` prints the body;
  - `-o FILE` saves it, within the program's scope; `-O` saves it under the URL's last segment;
  - `-c` resumes;
  - `-I` shows the head only;
  - `-X METHOD`, `-H 'Name: value'`, `-d TEXT` or `-d @FILE`, `-F name=@FILE` (multipart);
  - `-L` follows redirects (on by default, `--max-redirs N`);
  - `-s` is silent; `-w` prints the status and the time;
  - `--max-time S`;
  - `-u user` asks for the password on the console, never takes it from the command line.
- **How it reaches a host:**
  - HTTPS through the shell's TLS client, with `BADGE_TLS_NO_DEVICE`;
  - plain HTTP only with `--insecure-http`, said on the console.
- **Its authority:**
  - a flow grant for the URL's host and port, asked of the user at run time (000-NET-0049);
  - a VFS scope of the current directory for `-o`, `-d @` and `-F`;
  - nothing else.
- **The exit status** follows `curl`'s for the common cases (0 success, 6 no such host, 7 no connection, 22 an HTTP error with `-f`, 28 a timeout), so that scripts read it.
- **`fetch` and `https` stay** as they are; `get` is for everything beyond one GET.

## Acceptance criteria

- **Host tests:** the argument parser, the exit statuses.
- **In QEMU, x86 and aarch64** (a `get` suite against 000-NET-0047's stand-in server):
  - a GET to the console and to a file;
  - a resume after a cut;
  - a JSON POST from a file;
  - a multipart upload;
  - a redirect chain;
  - a refused certificate;
  - a host the user refuses.
- **The shell's `help get`** and docs/network.

## Progress (2026-10-28)

**Made** by the kernel track, for this open track's task (AGENTS.md section 5), with 000-NET-0045 and 000-NET-0049:

- **`get/`**, a process of its own (`get/src/args.rs`: the words split as a shell splits them, the options, curl's exit
  statuses; `get/src/main.rs`: the request). In `02_build.sh`'s programs, so in both images.
- **Options:** `-o`, `-O` (into `ram:`), `-c`, `-I`, `-X GET|POST|PUT`, `-H`, `-d TEXT|@FILE`, `-F name=value|name=@FILE`,
  `-L` (default), `--no-location`, `--max-redirs N` (10 at most), `-s`, `-w`, `-f`, `-m`/`--max-time`, `--insecure-http`.
  Credentials (`Authorization`, `Cookie`, `Proxy-Authorization`) are not sent on to another host or port after a
  redirect.
- **Its authority, apart from the plan:**
  - the files are the shell's client of the user's files (`REQUEST_FILES`: `ram:`, `data/`, `log:`), not a scope of the
    current directory: `REQUEST_FILE` scopes the directory of the program's first argument only, and `get`'s files may
    be any of its arguments;
  - the hosts are the policy's lines for `get` (a host and port, or a browsing grant, 000-NET-0049), not yet one asked of
    the user at run time.
- **Exit statuses:** 0, 1, 2, 3, 6, 7, 22, 23, 26, 28, 35, 47, 56, 60, curl's numbers.
- **Tested:**
  - host: `tests/get_host.rs` (5 tests: the words, every option, what is refused, `-O`'s name, the statuses), in
    `scripts/host_tests.sh`;
  - QEMU, x86 and aarch64, the `net` suite's download check (`get_runs`), against `tests/http_api_stub.py` (an echo, a
    redirect to the same host and to another, a slow answer added) and the release server over HTTPS. The list is in
    [docs/network/downloads.md](../docs/network/downloads.md#tested).
- **Docs:** [docs/network/downloads.md](../docs/network/downloads.md#get), README; `help get` is the program's own text.

**Left, so the issue stays open:**

- `-u user`, the password asked on the console;
- a host the user refuses at run time: 000-NET-0049's question, which waits for the shell (APP);
- `-I` asks with GET and reads no body, and `-X` has no HEAD, PATCH or DELETE: `mind::http::Method` has GET, POST and
  PUT only (000-NET-0047; effector's exhaustive `match` on it is recorded in `requests-EFF.md`).

## Progress (2026-10-29)

In `main` at ef3faf0d (landing round 41, the request's commit 3f4fa0aa): `get` and its host tests (`tests/get_host.rs`), and the `net` suite's `get_runs` against `tests/http_api_stub.py`, passed in the round 41 gate on x86 and aarch64 (the `net` suite runs in "x86: boot, display, network, TLS, shell, memory, clock", "aarch64: files, network and TLS" and the other aarch64 groups).

Still left: what the Progress section lists (`-u`, a host refused at run time, HEAD, PATCH and DELETE).

## Related

[000-NET-0047](../issues-done/000-NET-0047-http-for-api-clients.done), [000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md), [351-NET-0001](../issues-done/351-NET-0001-http-downloads.done).
