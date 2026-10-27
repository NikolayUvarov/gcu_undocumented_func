# 000-NET-0048 — `get`, an analog of `wget` and `curl`

**Type:** network (a new program `get`) · **Owner:** `NET` · **Priority:** P2 · **Status:** open · **Blocked by:** [000-NET-0047](000-NET-0047-http-for-api-clients.md), [000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md) (a host named at run time), [000-NET-0046](000-NET-0046-public-root-certificates.md) (public sites) · **Main task:** — · **Constitution:** MC-3.1, MC-11.6, MC-11.11, MC-12.1

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

## Related

[000-NET-0047](000-NET-0047-http-for-api-clients.md), [000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md), [351-NET-0001](../issues-done/351-NET-0001-http-downloads.done).
