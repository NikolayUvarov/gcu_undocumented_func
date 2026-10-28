# 261 — A mail client: IMAP and SMTP submission over TLS, in the manner of mutt

**Type:** main task, tools · **Owner:** `COM` track (moved from `APP` at the maintainer's word, 2026-10-25) · **Priority:** P2 · **Status:** open · **Blocked by:** where the password or token lives (as in [253](253-telegram-and-whatsapp-text-client.md), stage 0) · **Roadmap:** track G (user services), track D (Babel) · **Constitution:** MC-3.1, MC-3.8, MC-10.3, MC-11.4, MC-11.5, MC-11.6, MC-11.9, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: a mail program, beside the messengers.

## Problem

- **No program on MIND Core reads or sends mail.**
- **The protocols are standard and text-based:**
  - IMAP4rev2 (RFC 9051) to read;
  - message submission (RFC 6409) to send, with implicit TLS (RFC 8314);
  - the message format (RFC 5322) with MIME (RFC 2045–2049), encoded words (RFC 2047) and parameters (RFC 2231);
  - JMAP (RFC 8620, 8621) where a provider has it.
- **A message is untrusted input:**
  - HTML, attachments of any type;
  - remote images that tell the sender when it was read;
  - links that need not go where their text says.
- **Large providers** (Gmail, Outlook) want OAuth 2.0 (SASL `OAUTHBEARER`, RFC 7628, or `XOAUTH2`), or an application password.

## Plan

**The program `mail`** (its name is the maintainer's to change), on `mind::tui`, in the console and in a `wm` window.

**Authority:**
- flow grants for the account's IMAP and submission hosts on ports 993 and 465 only (MC-11.6);
- the shell's TLS client with `BADGE_TLS_NO_DEVICE`;
- a VFS scope of `data/apps/mail/` (its cache and settings) and of `data/media/` for saved attachments.

**Parsing:** MIME, HTML and headers run in a parser session that holds only the message (MC-11.11).

**1. Reading (IMAP4rev2, with IMAP4rev1 servers' common subset).**

- **Login:** `AUTHENTICATE PLAIN` over TLS, `OAUTHBEARER`/`XOAUTH2` with a token from the provider's OAuth flow (the user opens its URL on another device, or in the graphical browser, and pastes the code), or an application password.
- **Folders:** `LIST` with special-use folders (RFC 6154: Inbox, Sent, Drafts, Trash, Junk, Archive).
- **The message list:**
  - fetched by `UID FETCH` of envelopes and flags, threaded (`THREAD` where the server has it, else by `References` and `In-Reply-To`);
  - unread, flagged and answered marks.
- **Kept in sync** by `CONDSTORE`/`QRESYNC` (RFC 7162) where the server has them, else by `UIDVALIDITY` and UIDs. Pushed by `IDLE` (RFC 2177).
- **Search:** `SEARCH` on the server; the local cache for what was fetched.
- **A message shows:**
  - its headers (From, To, Cc, Date, Subject), decoded;
  - its text part, or its HTML reduced to text by the allow-list of 254 and 255;
  - remote images never fetched;
  - each link's real address beside its text.
- **Attachments** are listed with their types and sizes. `s` saves one into `data/media/`, and it opens in the program for its type (`view`, `calc`, `write`). An executable is never run from here.
- **Flags and moves:** read, flagged, deleted (`UID MOVE` to Trash, RFC 6851), archived, marked as junk.
- **An offline cache** in `data/apps/mail/` (envelopes and the messages read), with a stated size limit.

**2. Writing.**

- **Composing** in `edit` (or in place): To, Cc, Bcc, Subject from the address book, the body as `text/plain; format=flowed` (RFC 3676) in UTF-8.
- **Replies** quote the original; forwarding keeps attachments; attachments come from the program's scope.
- **Sending** by submission over implicit TLS (465), `AUTH` as for IMAP; the sent message is appended to Sent (`APPEND`).
- **A draft** is kept locally and in Drafts.
- **The `Message-ID` and `Date`** are made by the client; a send is never repeated for one keypress (MC-6.6).
- **The address book:** a local file of names and addresses, filled from messages read.

**3. Signing and encryption (later).**

- OpenPGP (RFC 9580) or S/MIME, with the private key kept so that the program can ask for a signature or a decryption but never read the key (MC-11.9).
- That needs a key operation of `keystore`'s owner (`NET`), asked when this stage starts.

**4. For scripts.**

- `mail send --to <addr> --subject <s> --body-file <path>`, and `mail check` (new messages' count) without a screen;
- a hardware test or a script can report by mail;
- the password or token from a file named on the command line, never the command line itself.

## Acceptance criteria

- **Host tests:**
  - the IMAP parser on recorded responses, cut at every byte;
  - MIME on a stated corpus (nested multiparts, encoded words in every header, RFC 2231 file names, broken messages);
  - the HTML reduction against hostile samples;
  - `format=flowed`.
- **A QEMU suite** against a local IMAP and submission server (a test server in `tests/`, TLS with the suite's CA):
  - logs in, lists, reads, searches and moves;
  - sends with an attachment and finds it in Sent;
  - keeps in sync after a change made by another client;
  - fetches no remote image.
- **On real providers:** the maintainer's accounts, with each provider and its authentication stated (MC-12.1).
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

## Related

[253](253-telegram-and-whatsapp-text-client.md) (secrets, media), [254](254-mastodon-text-client.md) and [255](255-text-web-browser.md) (HTML reduced to text), [257](257-full-text-editor.md) (composing).
