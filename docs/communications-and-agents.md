# Mail, Telegram, the web and AI agents: a plan across tracks

**Version:** 0.2 (2026-10-25): our own micro messenger, 950; 0.1: the kernel session as integrator, at the maintainer's request · **Status:** a plan; the tracks named below are the maintainer's to confirm (TRACKS.md) · **Constitution:** MC-3.1, MC-3.8, MC-8.1, MC-10.3, MC-11.4, MC-11.5, MC-11.6, MC-11.9, MC-11.11, MC-12.1, MC-12.3

## What is asked

The maintainer, 2026-10-25:

1. **Console utilities:**
   - `mail`, to send and read mail;
   - `tgtxt`, to work with Telegram;
   - an analog of `wget` and `curl`.
2. **Full-screen text programs:** a mail client, a web browser, a Telegram client.
3. **Graphical programs:** the next stage. For now only the intention is stated, below.
4. **A text agent module** that works with Claude, Gemini, Codex (OpenAI), Grok, Mistral and other well-known providers.
5. **A module that runs neural networks.**
6. **Our own micro messenger with a server:** the minimal functions of exchanging messages, in the text (full-screen) and the console modes (the same day).

## What there is to build on

- **Main tasks already recorded** by the tools track (`APP`) on 2026-10-23, none started yet:
  - [253](../issues/253-telegram-and-whatsapp-text-client.md) — Telegram (Bot API, then MTProto);
  - [254](../issues/254-mastodon-text-client.md) — Mastodon;
  - [255](../issues/255-text-web-browser.md) — a text browser, `www`;
  - [256](../issues/256-graphical-web-browser-current-standards-only.md) — a graphical browser;
  - [260](../issues/260-ai-agents-on-the-platform.md) — a terminal AI agent, `agent`;
  - [261](../issues/261-mail-client.md) — a mail client, `mail`;
  - [262](../issues/262-model-runtime-on-the-machines-full-hardware.md) — a model runtime, `nnd`.

  This plan follows them; it does not repeat their designs.
- **In the system today:**
  - the TLS 1.3 client service `tls`, which verifies servers against `tlsroots.pem` on the boot disk, where the image ships Mozilla's public roots (000-NET-0046);
  - `libmind`'s HTTP client (GET with resume, chunked bodies, keep-alive), `json` and `inflate`;
  - the `parse` service, for untrusted input in a session of its own (track D);
  - the network policy's flow grants by name (351-NET-0003);
  - `keystore`, which holds the device key and the remembered Wi-Fi networks;
  - `mind::tui` and `wm`'s windows;
  - `mind::nn`, ONNX on the CPU with SSE and AVX2;
  - for large models: large private memory, workers on several CPUs and large stacks (262-KRN-0160 to 0165).
- **Missing, and shared by every client:**
  - public root certificates in the image;
  - a place where a program keeps its token, password or API key;
  - a directory of a program's own;
  - HTTP for APIs: request bodies, multipart, redirects, compression, Server-Sent Events;
  - a way to reach a host the user names at run time.

## The stages

### F. Foundations (the kernel session: `KRN`, `NET`), first

| Task | What | Blocks |
|---|---|---|
| [000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done) | A program's own data directory and its identity, given by the loader by the program's name | the secrets, every client's settings and cache |
| [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done) | Secrets per program in `keystore`: a token, a password or an API key that only its program reads, never logged | `mail`, `tgtxt`, `agent` |
| [000-NET-0046](../issues/000-NET-0046-public-root-certificates.md) | Public root certificates in the image (Mozilla's set, MPL-2.0) | every client of a public service |
| [000-NET-0047](../issues/000-NET-0047-http-for-api-clients.md) | HTTP for API clients in `libmind`: bodies and headers, multipart, redirects, gzip, a streaming body, Server-Sent Events, Retry-After | `get`, `tgtxt`, `agent`, `www` |
| [000-NET-0048](../issues/000-NET-0048-get-a-wget-and-curl-analog.md) | `get`, the analog of `wget` and `curl` | — (a console utility of stage 1) |
| [000-NET-0049](../issues/000-NET-0049-a-flow-grant-asked-at-run-time.md) | A flow grant asked of the user at run time, and the browsing grant `www` needs | `get`, `www`, `mail` with a server the user names |

### 1. Console utilities

| Program | What | Track | Main task |
|---|---|---|---|
| `get` | `wget` and `curl`: methods, headers, bodies, multipart, resume, redirects, output to a file | `NET` | F above |
| `mail` | `mail send`, `mail list`, `mail read`, `mail check`: submission (RFC 6409, implicit TLS) and IMAP4rev2, MIME parsed in a parser session | `COM` | 261, its stage 4 first |
| `tgtxt` | `tgtxt send`, `tgtxt read [--follow]`, `tgtxt chats`, `tgtxt wait`: the Bot API (`getUpdates`, `sendMessage`, `sendDocument`, `getFile`), the update offset kept | `COM` | 253, stage 1 (`tgtxt` is the maintainer's name for 253's `tg`) |
| `ask` | one question to a language model, the answer streamed: `ask [-p provider] [-m model] [prompt]`, for scripts | `AGT` (or `COM`) | 260 |
| `msg` | our own messenger: `msg register`, `msg send`, `msg read [--follow]`, `msg contacts`, `msg wait`, against our server `msgd` | `COM` | [950](../issues/950-our-own-micro-messenger.md) |

### 2. Full-screen text programs (on `mind::tui`, in the console and in a `wm` window)

| Program | What | Track | Main task |
|---|---|---|---|
| `mail` | folders, threads, a message read as text, HTML reduced to text, remote images never fetched, a composer in `edit`, attachments, `IDLE` | `COM` | 261, stages 1 and 2 |
| `www` | Lynx's way: the WHATWG HTML parser in a parser session, text layout, numbered links, history, bookmarks; then forms and cookies by the user's leave | `COM` | 255 |
| `tgtxt` | the chat list, a chat, a line to write in; the bot first, then the user account through MTProto 2.0 | `COM` | 253, stages 1 and 2 |
| `msg` | with no arguments: conversations, a conversation, a line to write in, the "delivered" and "read" marks; the same widgets as `tgtxt` | `COM` | 950 |
| `agent` | the loop of Claude Code and Codex: the model's tool calls (read, find, edit with a diff and the user's yes, a command through the shell's client), permission modes, the audit | `AGT` (or `COM`) | 260 |

Shared by these: one HTML-to-text reducer for `mail` and `www`, MIME for `mail`, the widgets they all need from `mind::tui` (a list, a scrolling text view with search, an input line with history, a status line). The widgets are a request to `APP`, which owns `mind::tui`.

### 3. Graphical programs: the intention only

- The graphical browser [256](../issues/256-graphical-web-browser-current-standards-only.md) stays the tools track's (`APP`), long-term.
- `mail`, `tgtxt` and `agent` get a graphical face in a `wm` window later, on the same protocol modules. That waits for scalable fonts and text shaping, which [258](../issues/258-wysiwyg-document-editor.md) needs too.
- No task is recorded for this stage now.

### Our own messenger (main task [950](../issues/950-our-own-micro-messenger.md))

- **The protocol `mmp` 1:**
  - a crate of its own (`mmp/`, `no_std`), shared by the client, the server and the tests;
  - JSON over HTTPS, with Server-Sent Events for what arrives;
  - accounts made with an invitation, an Ed25519 key per account, a message sent again stored once.
- **The server `msgd`:**
  - a host program for Linux (`tools/msgd/`), with an append-only store;
  - messages kept until delivered, then for a stated time;
  - limits per account;
  - a log that never holds a message's text.
- **The client `msg`:** console commands first, then the full-screen screen.
- **What stage 1 is not:** in stage 1 the server's owner can read the messages. The connection is TLS, not end to end, and this is said plainly. End-to-end encryption, small files and `msgd` as a MIND service come later.
- **It stands on the same foundations:** 000-NET-0047 (HTTP with SSE), 000-KRN-0170 (its directory), 000-NET-0045 (its key), and a pinned key or 000-NET-0046 for the server.

### The agent module (main task 260)

- **One provider interface** in `libmind` (`llm`): messages, tools (function calls), streaming deltas, usage and errors, behind adapters:
  - Anthropic's Messages API (Claude);
  - OpenAI's Responses API and Chat Completions (the GPT and Codex models);
  - Google's Gemini API;
  - the OpenAI-compatible Chat Completions that xAI (Grok), Mistral, DeepSeek, OpenRouter and local servers (Ollama, vLLM, llama.cpp's server) offer;
  - the local runtime `nnd` (262), through the same interface, once it serves one.
- **Which provider, model, address and key name** come from a configuration file. No model is named in the code (as 260 says), and an API version is moved only by a change that says so (MC-12.4).
- **The programs on it:** `ask` (stage 1) and `agent` (stage 2).
- **Tests:**
  - a stand-in server per wire format (`tests/llm_stub.py`, streaming by SSE) in a QEMU suite;
  - a real provider's exchange on the maintainer's machine, named with its configuration (MC-12.1).
- **Keys:** from 000-NET-0045, never in a log, a command line or a session file (MC-10.3, MC-11.9).

### The module that runs neural networks (main task 262, with `NRL`'s 900 and 178)

- **262's runtime `nnd`:**
  - models in GGUF, safetensors and ONNX;
  - quantized matrix products (the 4-, 5- and 8-bit block formats) on AVX2, AVX-512 and NEON;
  - tokenizers: BPE and SentencePiece;
  - a decoder transformer with its KV cache, on workers on every CPU;
  - a service with an API that `llm` speaks (an OpenAI-compatible local endpoint).
- **The kernel's part:**
  - large memory, workers and stacks are done (262-KRN-0160 to 0165);
  - [262-KRN-0161](../issues/262-KRN-0161-large-pages.md), large pages, is open;
  - the accelerators of [178](../issues/178-models-on-every-accelerator.md) come later.
- **`NRL`** ([900](../issues/900-neural-representation-lab.md), open) is the lab around it: representation, conversion, fine-tuning.

### Our own web server (main task [110](../issues/110-our-own-web-server.md), `NET`)

Added on 2026-10-25 at the maintainer's request: the server side of HTTP, `httpd`, so that the project serves its updates itself. First a host program for Linux over the release directory; then a MIND Core service, with listening sockets in `netstack` and a server role in `tls`; later a static site and a base for `msgd` and for `improve`'s intake (100003).

## The tracks

| Track | Who | Takes |
|---|---|---|
| `KRN`, `NET` | the kernel session | the foundations F; 262-KRN-0161; the grants and slots every client needs, as their requests come |
| `COM` | **open** (confirmed by the maintainer, 2026-10-25): communication clients, main tasks 950–999; for the agent the maintainer connects | 950, our own messenger (`mmp`, `msgd`, `msg`); 253, 254 and 255, 261: the protocol modules (IMAP, submission, MIME, the Bot API, MTProto, the HTML reducer), `mail`, `tgtxt` and `www`, their suites and stand-in servers |
| `AGT` | **open** (opened by the maintainer, 2026-10-25): the agent module, main tasks 101000–101999, with 260 moved from `APP`; for the agent the maintainer connects | `llm`, `ask` and `agent` |
| `APP` | the tools session | the `mind::tui` widgets (a request), `wm`'s window for each program, the shell's commands that start them; 256 and the graphical stage |
| `NRL` | open: a candidate for another agent | 900, and 262's runtime `nnd` with `APP`'s agreement |
| `ASR` | the assessing session | fuzzing of the new parsers (MIME, HTML, SSE, the APIs' JSON, MTProto's TL), the threat model of programs that hold the user's secrets |

**The handover (the maintainer's word, 2026-10-25).** 253, 254, 255 and 261, recorded by `APP` and not started, move to `COM`; 260 moves to `AGT`. `APP` is told in a message on `internal-dev-messages`.

## The order

1. **First, together:**
   - `KRN`/`NET`: F (0170 and 0045 first, since the secrets wait for the identity; 0046, 0047 and 0049 in parallel);
   - `COM`: the mail protocol modules and the Bot API client, against stand-in servers, with tokens from a file until 0045 lands.
2. **Then:** `get` (`NET`), `mail`, `tgtxt` and `msg` as console utilities (`COM`), `msgd` with `msg`; `ask` (`AGT` or `COM`).
3. **Then:** the full-screen `mail`, `tgtxt`, `msg` and `www` once the `mind::tui` widgets are in; `agent` once `ask`'s provider layer holds.
4. **Then:** MTProto for `tgtxt`; forms and cookies for `www`; `nnd` behind `llm`.
5. **Later:** the graphical faces (stage 3).

**Every program, as 253 to 261 say:**
- its own flow grants and no other authority;
- the shell's TLS client with `BADGE_TLS_NO_DEVICE`;
- what the server sends is parsed in a parser session that holds nothing else (MC-11.11);
- what a tool, a page or a message says is data, never an instruction (MC-11.5).

## The brief for the agent that takes `COM`

Fill in [AGENTS.md](../AGENTS.md), section 7, with:

- **Track:** `COM`, communication clients. **Main tasks:** 950 (the messenger, its own), 253, 254, 255, 261, new ones from 951.
- **Counter:** from 0001.
- **Directories:**
  - new programs `mail/`, `tgtxt/`, `www/`, `msg/`; the protocol crate `mmp/` and the server `tools/msgd/`;
  - new `libmind` modules: `mail`, `mime`, `telegram`, `html`;
  - their `idl/` files if any;
  - `docs/communications/`;
  - stand-in servers under `tests/`;
  - their suites in `tests/qemu_smoke.py` and their groups in `scripts/ci_local.sh`.
- **What it needs from other tracks:**
  - F from `KRN` and `NET`: tokens come from a file until 000-NET-0045 lands;
  - the `mind::tui` widgets from `APP`;
  - flow grants from `NET`.

## The brief for the agent that takes `AGT`

Fill in [AGENTS.md](../AGENTS.md), section 7, with:

- **Track:** `AGT`, the agent module. **Main tasks:** 260 (moved from `APP`), new ones from 101000.
- **Counter:** from 0001.
- **Branch:** `<tool>/AGT-agents`.
- **Directories:**
  - the `libmind` module `llm`;
  - the programs `ask/` and `agent/`;
  - `docs/agents/`;
  - stand-in provider servers under `tests/` (`tests/llm_stub.py`), their suites in `tests/qemu_smoke.py` and their groups in `scripts/ci_local.sh`.
- **First:** `llm` with one wire format and its stand-in server, then `ask`; `agent` after them (260's stages).
- **What it needs from other tracks:**
  - HTTP with SSE and redirects from `NET` (000-NET-0047); keys from `keystore` (000-NET-0045: a program that asks for `REQUEST_SECRETS` keeps and reads its own in `SLOT_SECRETS`);
  - flow grants for the providers' hosts from `NET`;
  - its data directory from `KRN` (000-KRN-0170);
  - the `mind::tui` widgets from `APP`, for `agent`'s screen.

