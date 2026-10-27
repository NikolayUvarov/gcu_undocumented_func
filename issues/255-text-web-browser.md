# 255 — A text web browser, in the manner of Lynx

**Type:** main task, tools · **Owner:** `COM` track (moved from `APP` at the maintainer's word, 2026-10-25) · **Priority:** P2 · **Status:** open · **Blocked by:** — for stage 1; a cookie and form store per stage 2 · **Roadmap:** track G (user services), track D (Babel) · **Constitution:** MC-3.1, MC-3.8, MC-10.1, MC-11.3, MC-11.5, MC-11.6, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: a browser in two variants, a text one like Lynx and a graphical one ([256](256-graphical-web-browser-current-standards-only.md)).

## Problem

- **MIND Core fetches, but does not browse.**
  - The shell's `fetch` and `https` take one URL and print or save what comes.
  - Nothing reads HTML, follows a link or fills a form.
- **A text browser is the first way to read the web on the machine,** in the console, over the serial link and in a `wm` window. It needs no fonts beyond the 8x16 one and no script engine.

## Plan

**The program `www`** (its name is the maintainer's to change), on `mind::tui`.

**1. Reading.**

- **Fetching:**
  - HTTPS through the shell's TLS client, lent with `BADGE_TLS_NO_DEVICE`. Plain HTTP only after a question, with a warning line on every page.
  - HTTP/1.1 (RFC 9110, 9112): redirects, at most 10, each shown in the status line; `Content-Encoding: gzip` and `deflate` (`libmind` `inflate`); chunked bodies; `Content-Type` and its charset.
  - Only UTF-8 is read. A page in another charset is shown with its bytes as replacement glyphs, and says so, until the maintainer wants the WHATWG Encoding standard's legacy decoders.
- **Parsing** the HTML, in a parser session that holds only the page (MC-11.11):
  - the WHATWG HTML tokenizer and tree builder, so that broken pages give the same tree as in other browsers;
  - CSS is not applied; `display: none` and `hidden` are.
- **Laying out as text:**
  - headings, paragraphs, lists (numbered and not), definition lists, quotations and `pre`;
  - tables as text tables, or as a linear list when they are too wide;
  - `img` as `[alt]` or `[image]`; `iframe` as a link;
  - `noscript`'s content shown; scripts never run.
  - Long lines wrap to the window; the window's width and the font's cells decide.
- **Links:**
  - numbered, `[12]`;
  - ↑/↓ move between them, Enter follows, a number and Enter jumps;
  - ← goes back, → forward; `g` opens a URL.
- **Keys:**
  - `/` searches in the page, `n` the next match;
  - `=` shows the page's URL, title, size, type and certificate (its issuer and validity, from `tls`);
  - `\` shows the source;
  - `d` saves the page or a link's target into `data/media/`, through the program's VFS scope;
  - `q` quits, with a question.
- **History** for the session; **bookmarks** in `data/apps/www/bookmarks.html`.

**2. Forms and cookies.**

- **Forms:**
  - text fields, password fields (shown as dots and never logged), check boxes, radio buttons, `select`, `textarea` (edited in `edit` or in place);
  - submission by GET and POST, `application/x-www-form-urlencoded` and `multipart/form-data`, a file chosen from the program's scope.
- **Cookies** (RFC 6265 and its revision's `SameSite` rules):
  - kept per site in `data/apps/www/cookies`, only for the sites the user allows; asked the first time by default;
  - `Secure` and `HttpOnly` honoured;
  - third-party cookies never sent.
- **Authentication:** HTTP Basic, over HTTPS only.

**3. Other schemes.**

- `file:` within the program's VFS scope;
- `gopher:` and `gemini:`, if the maintainer wants them, as small separate parsers;
- `mailto:` shown, not followed.

## Authority

- **Flow grants:**
  - the user's browsing needs a grant for any host on ports 443 and 80;
  - that is wider than any program has today, so it is a request to `NET`'s broker when stage 1 starts: a grant for "any public address on these ports" with its own term and volume, and an audit line per new host (MC-10.3, MC-11.6).
- **Nothing else:** a VFS scope of `data/apps/www/` and of `data/media/` for saving; no lifecycle client, no other program's files.
- **The parser** holds only the page and gives back the tree; the program that holds the cookies does not parse.

## Acceptance criteria

- **Host tests:**
  - the HTML tokenizer and tree builder against the html5lib tests' tokenizer and tree-construction cases (a stated subset at first, then all; the count passed in the issue);
  - the text layout of fixed pages (lists, tables, `pre`);
  - cookies' rules from RFC 6265's test cases.
- **A QEMU suite** against a local HTTPS server with fixed pages:
  - follows links, a redirect and a 404;
  - submits a form by GET and POST;
  - keeps a cookie only after it is allowed;
  - saves a file;
  - refuses plain HTTP until asked.
- **On real sites:** a stated list (Wikipedia's article page, the project's GitHub page, a search engine's HTML page), with the date and what rendered (MC-12.1).
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

## Related

[256](256-graphical-web-browser-current-standards-only.md) (the graphical browser, which shares the HTML parser, the URL parser and the cookie store); [254](254-mastodon-text-client.md) (HTML reduced to text).
