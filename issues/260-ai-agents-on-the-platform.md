# 260 — AI agents on the platform: a terminal coding and operating agent that works as Claude Code and Codex do, over the providers' APIs

**Type:** main task, tools · **Owner:** `AGT` track (moved from `APP` at the maintainer's word, 2026-10-25) · **Priority:** P1 · **Status:** open · **Blocked by:** where the API key lives (as in [253](253-telegram-and-whatsapp-text-client.md), stage 0); a flow grant for the provider's host (`NET`'s broker, as today) · **Roadmap:** track G (user services); the cognitive plane (Article 8) · **Constitution:** MC-3.1, MC-3.8, MC-3.9, MC-3.11, MC-8.1, MC-10.2, MC-10.3, MC-11.4, MC-11.5, MC-11.9, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: support for AI agents on the platform, working as Codex and Claude do, through their APIs.

## Problem

- **An AI agent of the kind of Claude Code or the Codex CLI** is a program in a terminal that takes a task in words and runs a loop:
  1. it sends the conversation to a language model through the provider's API;
  2. the model answers with text or with calls of tools the program offers (read a file, search, edit a file, run a command);
  3. the program runs each call, within what the user allowed, and sends the results back;
  4. the loop goes on until the model ends its turn.
- **MIND Core has none.** It has what one stands on:
  - the TLS client, the network policy's flow grants, HTTP framing and JSON;
  - scoped file grants (VFS scopes);
  - the shell's command client, whose rule (`clients::taken_by`) already lets a lent client read at once and asks the user in the shell's window before anything changes the machine.
- **Not the same as the Effector agent** (750, `EFF`). That one serves a remote operator's commands under the owner's policy. This one acts for the user at the machine.
- **Not the system's "Mind"** (Article 8). It is an application with only the authorities it was given (MC-8.1).

## Plan

**The program `agent`** (its name is the maintainer's to change), on `mind::tui`, in the console and in a `wm` window.

**1. The providers, behind one interface in the program.**

- **Anthropic's Messages API:**
  - `POST https://api.anthropic.com/v1/messages`, with `x-api-key`, `content-type: application/json` and the `anthropic-version` header. The version is pinned in the program's configuration and moved only by a change that says so (MC-12.4).
  - **Streaming by Server-Sent Events:** `message_start`; `content_block_start`, `content_block_delta` (`text_delta`, `input_json_delta` for a tool call's arguments, `thinking_delta`) and `content_block_stop` per block; `message_delta` with the `stop_reason` and the usage; `message_stop`; `ping`; `error`.
  - **Tool use:**
    - `tools` with a name, a description and an `input_schema` (JSON Schema), `strict` where the schema must hold;
    - the model's `tool_use` blocks (an ID, a name, the input);
    - every call of one answer answered by `tool_result` blocks (the ID, the content, `is_error`) in one user message.
  - **Stop reasons** handled: `end_turn`, `tool_use`, `max_tokens`, `pause_turn` and `refusal`. The response is checked before its content is used.
  - **Prompt caching:** `cache_control` on the stable prefix (the tools, then the system prompt, then the earlier turns), so that a long session is not paid again for each turn.
  - **Context:** the model's context window comes from the Models API (`GET /v1/models/{id}`, `max_input_tokens`). Near it the session is compacted: the provider's compaction where it has one, else the program's own summary of earlier turns, said to the user.
  - **Errors:** 429 and 529 retried after `retry-after`, 5xx and cut connections with a backoff and a limit, 400s shown as they are. Token counting (`/v1/messages/count_tokens`) before a large request.
- **OpenAI's API** (the Responses API, or Chat Completions for servers compatible with it): the same loop, with its own streaming events and function calls.
- **A local model** through the model runtime of [262](262-model-runtime-on-the-machines-full-hardware.md), by the same interface, when the machine can run one.
- **The model** is the user's choice in the configuration, from the provider's model list. The program names no model in its code. A request's model, the tokens used and their cost (where the provider gives prices) are shown in the status line.

**2. The tools the agent offers the model.**

- **Read:** read a file (with a line range), list a directory, find files by a pattern (`find`), search contents (`grep`'s engine). Within the agent's VFS scope only.
- **Write:**
  - write a file;
  - edit a file by replacing an exact text (shown to the user as a diff before it is applied);
  - make a directory.
- **Run a command:**
  - a shell line through the shell's command client (`shell.wit` `run`), which already gives what reads at once and asks in the shell's window for what changes the machine;
  - its output and exit status come back to the model, truncated to a stated size.
- **The web:** fetch a page's text through the text browser's fetcher (255), read-only; the provider's own web search where the provider offers it and the user turns it on.
- **Ask the user** a question, and **a task list** the model keeps and the screen shows.

**3. Authority (MC-3.1, MC-3.8, MC-8.1, MC-11.4).**

- **What the agent holds,** and nothing more:
  - a VFS scope of one directory, the project, chosen at start (`agent <dir>`);
  - the shell's command client lent to it, under the lent clients' rule;
  - a flow grant for its provider's host on port 443 only;
  - the shell's TLS client with `BADGE_TLS_NO_DEVICE`.
  - No lifecycle client, no `mode`, no network policy change, no other directory.
- **Permission modes,** shown in the status line:
  - **ask** (the default): every write and every command that changes something is shown and waits for the user's yes;
  - **allow for this session:** a tool or a command the user allowed once;
  - **plan:** read only, the model proposes and nothing is written.
  - The user can stop any turn with Esc.
- **Untrusted input.** What the tools return (files, command output, web pages) is data, never instructions to the program (MC-11.5). A text that asks the agent to do more gets no authority it did not have: the capabilities above are the bound, whatever the model is told (MC-3.9).
- **The audit:** the system log names each tool call that changed something, with its arguments, and the user's answer (MC-10.3). The API key is never in a log, a command line or a session file.

**4. Sessions.**

- **The project's instructions:** a project file (`AGENTS.md`, or `MIND.md`) is read at start into the system prompt, as other agents read theirs.
- **Sessions** are kept in `data/apps/agent/sessions/` and resumed (`agent --resume`).
- **Without a screen:** `agent -p "<task>"` runs one task, prints the result as text or JSON, and exits with a status. Its permission mode is set on its command line, and nothing that changes the machine runs in it unless `--allow` names it.

**5. Later.**

- Sub-agents for parallel reading;
- the Model Context Protocol's servers as further tools (each one's authority stated);
- voice: dictation in, speech out, through 250 and 252.

## Acceptance criteria

- **Host tests:**
  - the SSE parser on recorded streams of each provider, cut at every byte;
  - the tool loop on a recorded transcript: parallel calls answered in one message, an error result, `max_tokens` and `refusal` handled;
  - the permission rule for every tool.
- **A QEMU suite** against a stand-in provider (`tests/llm_stub.py`, HTTPS with the suite's CA). It speaks the Messages API's streaming with tool calls:
  - the agent reads a file, proposes an edit, is told no, then yes;
  - it runs a command through the shell's window;
  - a tool result that asks for more authority gets nothing.
- **On the maintainer's key:** a stated task in a stated project on QEMU and on the MacBook Pro, with the provider, the model the user chose and the tokens stated (MC-12.1).
- **Docs:** `docs/tools/README.md` and `README_RU.md`, and a page on the agent's authority in `docs/profile/` once the code exists.

## Related

[750](750-effector-agent.md) (the remote agent), [262](262-model-runtime-on-the-machines-full-hardware.md) (local models), [255](255-text-web-browser.md) (fetching), [800-APP-0102](800-APP-0102-the-shells-run-for-the-links-client.md) (what a lent client gets), [071](../issues-done/071-scoped-file-grants.done) (VFS scopes).
