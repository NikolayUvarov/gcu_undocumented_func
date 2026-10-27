# 253 — A text messenger: Telegram through the Bot API and the user API, and WhatsApp where an API allows it

**Type:** main task, tools · **Owner:** `COM` track (moved from `APP` at the maintainer's word, 2026-10-25) · **Priority:** P2 · **Status:** open · **Blocked by:** stage by stage, below · **Roadmap:** track G (user services), track D (Babel: session parsers with minimal authority) · **Constitution:** MC-3.1, MC-3.8, MC-10.3, MC-11.4, MC-11.6, MC-11.9, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: a text client for Telegram, with the bot and the user API, and WhatsApp if an API is available.

## Problem

- **No messenger runs on MIND Core.** The system has the pieces such a client stands on:
  - the TLS 1.3 client service `tls`;
  - the network policy broker, whose flow grants name each program's destinations;
  - `libmind`'s HTTP framing and JSON;
  - the `parse` service for untrusted input;
  - `mind::tui` and `wm`'s windows.
- **There is no program that talks to a messaging service,** and no place where a program keeps its own login token.
- **What the three services offer differs:**
  - **The Telegram Bot API** is HTTPS and JSON (`https://api.telegram.org/bot<token>/<method>`). Updates come by long polling (`getUpdates`) or by a webhook. A bot sees only the chats it is in, and cannot start a chat with a user.
  - **The Telegram user API** is MTProto 2.0, a binary protocol over TCP. It needs:
    - an application's `api_id` and `api_hash` from my.telegram.org, and the user's phone number, login code and, if set, the cloud password (SRP);
    - its own cryptography: AES-256-IGE, SHA-256, RSA and Diffie–Hellman for the authorization key, and a factorization of `pq`;
    - the TL schema, a large and versioned ("layer") description of every type and method.
    - Telegram's API Terms of Service apply to every application that uses it.
  - **WhatsApp** has no API for a personal account. Its only official API is the WhatsApp Business Platform (the Cloud API, HTTPS and JSON, through Meta), for a business phone number.
    - Messages arrive only by a webhook, a public HTTPS endpoint, which a machine behind NAT does not have.
    - Unofficial clients of the personal protocol break WhatsApp's terms, and are not considered here.

## Plan in stages

**0. What every stage shares.**

- **One program per service,** each its own process:
  - `tg` for Telegram (the bot and the user modes);
  - `wa` for WhatsApp (stage 4).
- **The authority of each:**
  - its own flow grants, `netpolicy` lines naming only its service's hosts and ports (MC-11.6);
  - the shell's TLS client, lent with `BADGE_TLS_NO_DEVICE` so that it cannot sign in as the device;
  - a VFS scope of its own data directory, `data/apps/<program>/`.
  - Nothing else: no lifecycle client, no other program's files.
- **Parsing untrusted input** (JSON, TL frames, media headers) runs in a parser session that holds only its input and output (MC-11.11). The program that holds the token does not parse what the server sends.
- **Secrets:** the bot token, the user's authorization key, the WhatsApp access token.
  - They are kept so that only that program can read them, and never in a log or a command line (MC-10.3, MC-11.9).
  - Where they live is a request to the owners of `keystore` (`NET`) or of a program's private directory (`KRN`), made when stage 1 starts: a secret per program in `keystore`, or a directory `init` gives a program as it gives services.
  - Until then the client asks for the token at start and keeps it in memory only.
- **The screen:** `mind::tui`, in the console and in a `wm` window.
  - The chat list, with unread counts.
  - A chat's messages, newest at the bottom, with authors, times, replies and edits.
  - A line to write in, with Enter to send and Shift+Enter for a new line.
  - Search in a chat.
- **Text:** UTF-8 throughout. What the 8x16 font cannot draw (emoji, CJK) shows as its code, `<U+1F600>`, until the font grows; that is a request to `KRN`'s font, not a loss of the character.
- **Media:**
  - shown as `[photo 1280x960, 211 KB]`, `[voice 0:14]`, `[file report.pdf]`;
  - `d` downloads one into `data/media/`;
  - a picture opens in `view` or the graphical browser's image view once there is one;
  - sending a file is `/send <path>`, from what the program's scope lets it read.
- **Notifications:** a line in `wm`'s top bar and a tone (`beep`), each switchable.

**1. Telegram, the Bot API.**

- **Calls:** `getMe`, `getUpdates` (long polling with a timeout), `sendMessage` (with `reply_to_message_id`, `parse_mode` off by default), `editMessageText`, `deleteMessage`, `sendDocument`/`sendPhoto` (`multipart/form-data`), `getFile`, `answerCallbackQuery`.
- **Messages:** inline keyboards are shown as numbered buttons.
- **The update offset** is kept in the data directory, so that a restart neither repeats nor loses an update.
- **For scripts:** `tg bot send <chat> <text>` and `tg bot wait` without a screen, so that a hardware test or a script can report through a bot. The token is read from a file named by `--token-file`, as `wifi join` reads its passphrase.
- **Rate limits:** HTTP 429 with `retry_after` is obeyed; a message is never sent twice for one Enter.
- *Acceptance:*
  - host tests: the JSON of each call and update, the offset, a 429;
  - a QEMU suite against a local stand-in of the Bot API (`tests/telegram_bot_stub.py`, HTTPS with the suite's test CA) that sends, receives, edits and downloads;
  - on the maintainer's machine, a real bot's exchange, named with its configuration (MC-12.1).

**2. Telegram, the user API (MTProto 2.0).**

- **Transport:** TCP with the intermediate or the padded intermediate framing, to the data centres' addresses. Those are listed in the program's flow grants, and the list comes from `help.getConfig`.
- **Cryptography,** in `no_std` Rust from audited crates where they exist (RustCrypto: `aes`, `sha2`, `rsa`, `num-bigint`), recorded in THIRD_PARTY.md:
  - the authorization key's creation (`req_pq_multi`, `req_DH_params`, `set_client_DH_params`), with every check MTProto's documentation requires of a client: the server's nonce, the prime and the generator, the range of `g_a` and `g_b`;
  - message keys, the `msg_id` and `seq_no` rules, salts, acknowledgements, and resending after `bad_server_salt` and `bad_msg_notification`.
- **The TL schema:** a generator on the host turns `api.tl` and `mtproto.tl` of one named layer into Rust types and their serialization (like `scripts/mind_idl.py` for WIT). The generated code is committed; the layer is named in the program's `--version`.
- **Login:**
  - `auth.sendCode`, `auth.signIn`, the cloud password through `account.getPassword` and SRP (`auth.checkPassword`);
  - a QR login (`auth.exportLoginToken`) as the alternative, shown as a QR code in a `wm` window.
- **Chats:**
  - `messages.getDialogs`, `messages.getHistory`, `messages.sendMessage`, `updates.getDifference` after a gap;
  - `messages.readHistory`, `messages.editMessage`, `messages.deleteMessages`;
  - `upload.getFile` and `upload.saveFilePart` in parts, from the right data centre (`FILE_MIGRATE_X`).
- **Left out at first, and named so in its help:**
  - secret chats (end-to-end, MTProto's own layer);
  - calls;
  - stories;
  - the API's channels' administration.
- *Acceptance:*
  - host tests: the published test vectors of MTProto's key derivation and AES-IGE, and the TL serialization of a sample of the schema's types;
  - a session against Telegram's test data centres (the test servers MTProto's documentation names) with a test phone number: login, a sent and a received message, a file;
  - on the maintainer's account only with the maintainer's own `api_id`, and with what was run stated.

**3. Both modes together.**

- `tg` holds several accounts, a bot or a user each, and switches between them (`Ctrl+N`).
- Each account has its own data directory and secret.

**4. WhatsApp, the Business Platform's Cloud API (only if the maintainer wants a business number).**

- **Sending:** `POST /<version>/<phone-number-id>/messages` on `graph.facebook.com`: text, a template message (needed to open a conversation after 24 h), media by an uploaded ID.
- **Receiving needs a webhook relay,** because the machine has no public endpoint.
  - A small relay server, under the maintainer's domain, verifies Meta's signature (`X-Hub-Signature-256`) and keeps the messages.
  - `wa` fetches them over HTTPS, under the relay's pinned key (`tls.wit` `connect-pinned`).
  - The relay is part of this stage, with its own threat model, and holds no WhatsApp token.
- **If the maintainer does not want a business account,** this stage is closed as not done, with this reason.
- *Acceptance:* a message sent and one received through a test number of Meta's developer account; the relay's signature check refusing a changed body.

## Acceptance criteria

Each stage's criteria above, in order. The main task closes when stages 1 to 3 pass and stage 4 is done or closed by the maintainer's decision.

## Related

- [351-APP-0017](../issues-done/351-APP-0017-shell-lends-the-tls-client.done) (the TLS client the shell lends);
- [254](254-mastodon-text-client.md) (Mastodon, which shares the screen and the secrets);
- `idl/netpolicy.wit`, `idl/keystore.wit`, `idl/parse.wit`; [requests-NET.md](requests-NET.md), [requests-KRN.md](requests-KRN.md).
