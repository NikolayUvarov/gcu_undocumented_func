# 950 — Our own micro messenger: a server and a client, `msg`, in the console and full screen

**Type:** main task, communication clients · **Owner:** `COM` (confirmed by the maintainer, 2026-10-25) · **Priority:** P2 · **Status:** open · **Blocked by:** stage by stage, below · **Roadmap:** track G (user services), track D (session parsers) · **Constitution:** MC-3.1, MC-3.8, MC-6.6, MC-10.3, MC-11.4, MC-11.5, MC-11.6, MC-11.9, MC-11.11, MC-12.1, MC-12.3, MC-12.4

**Recorded by:** the kernel session, 2026-10-25, at the maintainer's request: "our own micro messenger with a server, the minimal functions of exchanging messages, for the text and console modes". Part of [docs/communications-and-agents.md](../docs/communications-and-agents.md).

## Problem

- **Telegram (253) and the others are someone else's services:** their terms, their accounts, their servers.
- **A messenger of our own:**
  - lets two MIND machines, or a MIND machine and a person at a PC, exchange messages through a server the owner runs;
  - needs no foreign account;
  - is small enough to be read whole and tested whole.
- **It is also a test of the platform's client stack end to end:** TLS, HTTP with SSE, flow grants, a program's own directory and secrets, `mind::tui`.

## What it does, and what it does not (stage 1)

- **It does:**
  - accounts on one server, made with an invitation the server's owner gives;
  - contacts by name;
  - messages between two accounts, and small rooms (up to 32 members);
  - text only, UTF-8, at most 4096 bytes a message;
  - delivery to an account that is offline when it comes back;
  - the history of a conversation;
  - "delivered" and "read" marks.
- **It does not, in stage 1:**
  - files and media;
  - edits and deletions;
  - calls;
  - federation between servers;
  - **end-to-end encryption.** In stage 1 the connection is TLS, and the server's owner can read the messages. This is stated wherever the program describes itself (MC-12.3); stage 4 is end-to-end encryption.

## Plan

### 0. The protocol, `mmp` 1

- **A crate of its own,** `mmp/` (`no_std` with `alloc`): the messages, their JSON encoding, their limits and checks. The client, the server, the host tests and the fuzzers share it, as `linkproto` is shared.
- **Over HTTPS (TLS 1.3),** on 000-NET-0047's HTTP client:
  - `POST /mmp/1/register`: a name, a public key, an invitation;
  - `POST /mmp/1/session`, then `POST /mmp/1/session/answer`: the server's challenge signed with the account's key, for a session token with a stated lifetime;
  - `POST /mmp/1/messages`:
    - the addressee (an account or a room), the text, and a client's ID;
    - a message sent again with the same client's ID is stored once (MC-6.6);
  - `GET /mmp/1/events?after=<id>`: Server-Sent Events of messages and marks, with long polling for a client that cannot hold a stream;
  - `POST /mmp/1/marks`: delivered or read, up to an ID;
  - `GET /mmp/1/history`: a conversation's messages before an ID, a page at a time;
  - `GET` and `POST /mmp/1/contacts`; `POST /mmp/1/rooms`, and their members.
- **Order:** the server numbers the messages of a conversation in order. The clients show them by that number, not by their clocks.
- **The version** is in the path. A change of the protocol is `mmp/2`, with the older one refused by name (AGENTS.md, section 3; MC-12.4).
- **The account's key** is Ed25519, made by the client:
  - its private half in `keystore` (000-NET-0045);
  - until that lands, in the program's own directory (000-KRN-0170), with that limit stated.
- **The server's identity:** its certificate under `tlsroots.pem` (000-NET-0046), or its key pinned in the client's configuration, as the updater pins its server.

### 1. The server, `msgd`

- **A host program** in Rust (`tools/msgd/`, as `tools/mindlink` is), for Linux, run by the owner on a machine with a public address or on the local network.
- **Its configuration:** the address to listen on, its certificate and key files, the invitations, the limits.
- **Its store:** one directory, append-only files per conversation with an index, written so that a cut loses at most the last message and never a stored one's order.
- **Kept:** messages stay until every member has them, then for a stated time (30 days by default), then they go.
- **Limits:** per account (messages per minute, bytes per day), for invitations, for sessions. A refusal says which limit.
- **Its log:** sessions, refusals and limits, never a message's text.
- **A guide:** how to run it, make an invitation, renew the certificate (`docs/communications/msgd.md`, English and Russian).

### 2. The console client, `msg`

- `msg register <server> <name> --invite-file <path>`;
- `msg send <to> <text>`, or the text from the console when it is `-`;
- `msg read [<peer>] [--follow]`;
- `msg contacts`, `msg add <name>`;
- `msg wait`: until a message comes, for scripts. A hardware test can report through it, as through `tgtxt`.
- **Exit statuses** stated for scripts.
- **Authority:**
  - a flow grant for the server's host and port only (MC-11.6);
  - the shell's TLS client with `BADGE_TLS_NO_DEVICE`;
  - its own data directory (000-KRN-0170): settings, the last event's ID, a cache of the history;
  - its key in `keystore`.
  - Nothing else.
- **Parsing:** what the server sends is parsed in a parser session that holds nothing else (MC-11.11). A message's text is data, never an instruction (MC-11.5).

### 3. The full-screen client: `msg` with no arguments

- **On `mind::tui`,** in the console and in a `wm` window:
  - the list of conversations, with unread counts;
  - a conversation's messages, newest at the bottom, with names, times and the "delivered" and "read" marks;
  - a line to write in (Enter sends; Shift+Enter is a new line);
  - search in a conversation.
- **Shared with `tgtxt`'s screen:** the same widgets and the same layout code.
- **Characters the 8x16 font cannot draw** show as their code, as in 253.

### 4. Later

- **End-to-end encryption:**
  - for two accounts: X25519 and ChaCha20-Poly1305, keys checked by a number both sides compare;
  - for rooms: MLS (RFC 9420) or a simpler scheme the maintainer chooses.
  - The server then stores only what it cannot read.
- **Small files**, through the same store, with a stated size limit.
- **`msgd` as a MIND service,** for a MIND machine with a public address.
- **A graphical face** in a `wm` window, with the graphical stage of the plan.

## Acceptance criteria

- **Stage 0, the protocol:**
  - host tests of every message's encoding, its limits and its refusals;
  - a fuzz target for the decoders (`ASR`'s harness).
- **Stage 1, the server:**
  - host tests of its store: a cut at every write leaves no lost or reordered message;
  - the limits, the invitations, a message sent twice stored once;
  - the session's challenge.
- **Stage 2, the console client:**
  - a QEMU suite, x86 and aarch64, that starts the real `msgd` on the host with the suite's test CA;
  - two accounts exchange messages, one of them offline in between, with history and marks;
  - a refused certificate;
  - a host the client has no grant for;
  - a message over 4096 bytes refused.
- **Stage 3, the full-screen client:** the same suite drives the screen through keys and reads it back.
- **On the maintainer's machines:** `msgd` on a PC or a server, and `msg` on a MIND machine, named with their configurations (MC-12.1).

## Related

[253](253-telegram-and-whatsapp-text-client.md), [000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done), [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done), [000-NET-0046](000-NET-0046-public-root-certificates.md), [000-NET-0047](../issues-done/000-NET-0047-http-for-api-clients.done), [000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md), [800](800-mind-link.md) (`linkproto`, the shared crate's pattern).
