# 254 — A text client for Mastodon

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** where the token lives (as in [253](253-telegram-and-whatsapp-text-client.md), stage 0) · **Roadmap:** track G (user services), track D (Babel) · **Constitution:** MC-3.1, MC-3.8, MC-10.3, MC-11.4, MC-11.5, MC-11.6, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: the Mastodon messenger, beside the Telegram client.

## Problem

- **Mastodon is a federated network.** A user's account lives on one server (an "instance"). The client speaks to that server only, over its REST API:
  - HTTPS and JSON, under `/api/v1/` and `/api/v2/`;
  - OAuth 2.0 for the login;
  - a streaming API: Server-Sent Events, or a WebSocket, at `/api/v1/streaming`.
- **MIND Core has the HTTPS, JSON and parser pieces** (see 253), but no client for it.
- **The input is untrusted:**
  - a post's content is HTML that any server of the federation may have written;
  - its media and its link previews come from other servers.

## Plan

**The program `mast`** runs in the console and in a `wm` window, on `mind::tui`. It has the same authority as 253's clients:
- flow grants for its one instance's host (and its media host, which the instance names);
- the shell's TLS client, `BADGE_TLS_NO_DEVICE`;
- a VFS scope of `data/apps/mast/`.

**1. Login.**

- **OAuth 2.0 with PKCE:**
  - `POST /api/v1/apps` registers the client with the instance (scopes `read write follow push`; `profile` where the server has it);
  - the user opens the authorization URL on another device, or in the graphical browser once there is one, and pastes the code back (`urn:ietf:wg:oauth:2.0:oob`);
  - `POST /oauth/token` gives the access token.
- **Or a token** the user made in the instance's settings, read from a file (`--token-file`).
- **The token** is kept as 253 decides, never in a log or a command line (MC-10.3). `POST /oauth/revoke` on log-out.

**2. Reading.**

- **Timelines:**
  - home, local and federated (`/api/v1/timelines/...`);
  - a list, a hashtag;
  - a profile's posts;
  - bookmarks and favourites.
- **Paging** by the `Link` header's `max_id`/`min_id`, never by guessing.
- **Notifications** (`/api/v2/notifications`, grouped where the server supports it): mentions, boosts, favourites, follows, polls ended.
- **Live updates** from the streaming API's `user` and `public` streams: SSE over the same HTTPS client. A WebSocket only if the server has no SSE.
- **A post shows:**
  - its author (`display name @user@instance`) and time;
  - its content warning, folded until Enter;
  - its text: the HTML reduced to text by an allow-list (paragraphs, line breaks, links, mentions, hashtags, `code`). Every other tag is dropped, and its text kept;
  - its media as `[image 1200x800: <alt text>]`, `[video 0:31]`, `[audio]`, opened as 253's media are;
  - its poll, its boosts, favourites and replies, and the thread on Enter.
- **Custom emoji** show as `:name:`.

**3. Writing.**

- **A post:** `POST /api/v1/statuses`:
  - its text, counted against the instance's limit (`/api/v2/instance` `configuration.statuses.max_characters`);
  - visibility (public, unlisted, followers only, direct);
  - a content warning, a language;
  - media with alt text (`POST /api/v2/media`, a progress line until it is processed);
  - a poll, a reply.
- **An idempotency key** (`Idempotency-Key`) per post, so that a retry after a cut never posts twice (MC-6.6).
- **On a post:** edit, delete, boost, favourite and bookmark it.
- **On an account:** follow, unfollow, mute and block it, and report it.
- **Search:** `/api/v2/search` for accounts, hashtags and posts.

**4. For scripts.**

- `mast post <text>`, `mast post --file <path>` and `mast notifications --since <id>` without a screen.
- They exit with 0, 1 or 2, as `wifi join` does.

## Acceptance criteria

- **Host tests:**
  - the JSON of the entities used (Status, Account, Notification, MediaAttachment, Poll, Instance);
  - the HTML reduction against hostile samples (scripts, nested tags, entities, bidirectional controls);
  - the `Link` paging;
  - the SSE framing.
- **A QEMU suite** against a local stand-in of the API (`tests/mastodon_stub.py`, HTTPS with the suite's CA) that logs in with OAuth, reads a timeline, streams a new post, posts with media and an idempotency key, and edits and deletes.
- **On a real instance,** with the maintainer's test account: the same steps, with the instance and the version stated (MC-12.1).
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

## Related

[253](253-telegram-and-whatsapp-text-client.md) (the screen, the media and the secrets it shares); [255](255-text-web-browser.md) (the HTML reduction it may share with the text browser).
