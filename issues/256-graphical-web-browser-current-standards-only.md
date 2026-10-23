# 256 — A graphical web browser that implements only the current standards, with no legacy and no backward compatibility

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P3 (long-term) · **Status:** open · **Blocked by:** stage by stage, below · **Roadmap:** track G (user services), track D (Babel) · **Constitution:** MC-3.1, MC-3.3, MC-5.1, MC-5.2, MC-6.1, MC-10.1, MC-11.2, MC-11.3, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: a graphical browser beside the text one ([255](255-text-web-browser.md)), supporting only the latest RFCs and standards, with no legacy support and no backward compatibility.

## Problem

- **A graphical browser is the largest program the system could run.** Its parts are of the size of an operating system's:
  - HTML, CSS and their layout;
  - a JavaScript engine and the DOM's and the web platform's APIs;
  - images, fonts and media;
  - networking and security.
- **The maintainer's rule cuts its size:** only the current version of each standard, and none of what the web keeps for old pages. That rule has to be written down precisely, or it means nothing: below is the list, for the maintainer to confirm or change.
- **MIND Core today has:**
  - pixel windows through `wm` (`window.wit`);
  - PNG, JPEG and BMP decoders in `libmind`;
  - TLS 1.3 through `tls`, HTTP/1.1 framing, the `parse` service;
  - one 8x16 bitmap font;
  - one 64 KiB stack per task and no threads; at most 1 GiB of memory per application.
- **What it does not have:**
  - scalable fonts, text shaping or bidirectional text;
  - a script engine;
  - HTTP/2 or HTTP/3;
  - audio and video codecs for the web;
  - threads.

## What "current standards only" means (for the maintainer to confirm)

**Supported, each in its current version** (the WHATWG living standards as of a named date; the newest RFC and W3C Recommendation or Candidate Recommendation snapshot):

- **Protocols:**
  - HTTP semantics (RFC 9110), HTTP/1.1 (RFC 9112) as the minimum, HTTP/2 (RFC 9113), HTTP/3 and QUIC (RFC 9114, 9000) later;
  - TLS 1.3 (RFC 8446) only;
  - the WHATWG Fetch, URL, MIME Sniffing and Encoding standards, cookies by RFC 6265's revision (`SameSite=Lax` by default).
- **Documents:** the WHATWG HTML and DOM standards, in no-quirks mode only.
- **Styles:**
  - CSS by the W3C's current snapshot: selectors level 4, the cascade with layers, custom properties, `calc()`;
  - flexbox, grid, `@media` and `@container`;
  - logical properties, `color()` and the CSS colour spaces, `@font-face` with WOFF2.
- **Scripts:**
  - ECMAScript of the latest edition, in strict mode semantics wherever the language lets a host choose;
  - modules (`type="module"`);
  - the platform's APIs a stated page set needs, added one by one: the DOM, events, `fetch`, `URL`, timers, `Promise`, `localStorage`, `history`, `requestAnimationFrame`, `canvas` 2D.
- **Images:** PNG, JPEG, WebP, AVIF and SVG (as images).
- **Fonts:** WOFF2 and OpenType.
- **Security:**
  - HTTPS only, with HSTS and upgrade of every subresource;
  - Content Security Policy level 3, the same-origin policy, CORS;
  - `Cross-Origin-Opener-Policy` and `-Embedder-Policy`;
  - one process per site (below).

**Not supported, on purpose, and said so on the page that needs it:**

- quirks and limited-quirks modes (a document without `<!DOCTYPE html>` is laid out as no-quirks);
- the HTML standard's obsolete features (its section "Obsolete features": `frameset`, `font`, `center`, `marquee`, `applet` and the like render as unknown elements), `document.all`, `document.write` after load;
- ECMAScript's Annex B (the web's legacy additions: `__proto__` as an accessor, HTML-like comments, legacy octal literals, `escape`/`unescape`, `RegExp.$1`), sloppy-mode-only behaviour where the host may refuse it, `with`;
- legacy encodings: only UTF-8 is decoded;
- plain HTTP, TLS 1.2 and older, SHA-1 certificates, mixed content;
- third-party cookies, `document.domain`;
- plugins (NPAPI, Flash, Java applets), XSLT, MathML's deprecated parts, vendor prefixes (`-webkit-`).

## Plan in stages

**0. The architecture** (written as a design page before code, `docs/browser/design.md`):

- **Processes:**
  - **a browser process** holds the window, the address bar, the network grants, the cookie store and the history;
  - **one content process per site** (scheme and registrable domain) parses, styles, lays out and runs scripts. It holds only an IPC channel to the browser process and its own memory budget (MC-3.3, MC-5.1, MC-11.11). It has no network, no files and no window of its own: the browser process fetches for it, checks every request against the page's origin and CSP, and composes its pixels;
  - **image and font decoding** in a decoder session that holds only the bytes (MC-11.11).
- **A content process that hangs or exceeds its budget** is ended and its tab says so (MC-5.2, MC-6.1).
- **The engine's parts are crates of their own**, `no_std` + `alloc`, tested on the host: URL, HTML, CSS, style, layout, paint, JS, DOM bindings.
  - Third-party engines (Servo's crates, Boa, html5ever, stylo) are evaluated for `no_std` and their size.
  - Each one used is recorded in THIRD_PARTY.md with its licence; the evaluation is a page of the design.

**1. Static pages, no scripts.**

- Fetching as 255 does, then HTTP/2.
- The HTML parser (shared with 255).
- CSS parsing, the cascade and computed styles.
- Block, inline, flex and grid layout; scrolling, selection, links, find in page.
- Images: PNG and JPEG from `libmind`, then WebP, AVIF and SVG.
- **Text:**
  - scalable fonts (an OpenType rasterizer, for example `fontdue` or `ab_glyph`, both `no_std` with `alloc`);
  - shaping (`rustybuzz`);
  - the bidirectional algorithm (UAX #9) and line breaking (UAX #14);
  - fonts bundled under their licences (for example Noto) on the boot disk or the model disk.
- *Acceptance:* the Web Platform Tests' reftests of a stated set of CSS and HTML directories, the pass count recorded; a stated set of real sites read correctly; a page's memory and time measured.

**2. Scripts.**

- The ECMAScript engine (evaluated in stage 0; Boa is a candidate), without Annex B.
- The DOM and events, the platform's APIs listed above, each with its Web Platform Tests directory.
- **Budgets:** a script's time per frame, its memory per process, and timers' minimum interval; a page over its budget is stopped and says so.
- *Acceptance:* Test262's results for the engine (the pass count by feature), the WPT directories of the APIs added, and a stated set of sites that need scripts.

**3. Forms, storage, downloads and the browser's own screens.** Tabs, history, bookmarks, the certificate view, the site's permissions, downloads into `data/media/`.

**4. Media and later protocols.** HTTP/3, `<video>` and `<audio>` with royalty-free codecs (AV1, Opus, VP9) if the hardware is fast enough.

**Needed from other tracks, asked when each stage starts:**

- `KRN`: threads or several stacks per task for the content process, larger stacks than 64 KiB, shared memory between the content and browser processes for pixels;
- `NET`: a broad grant for browsing (as in 255), and QUIC's UDP for HTTP/3;
- `DRV`: audio for media.

## Acceptance criteria

Each stage's criteria above, in order. The main task is split into `APP` tasks per stage when stage 0's design is agreed. Results are stated as WPT and Test262 pass counts on the configurations run, never as "supports HTML" (MC-12.1, MC-12.3).

## Related

[255](255-text-web-browser.md) (the text browser: the HTML parser, URL, fetching and cookies shared); [650](650-building-on-the-target.md) (a POSIX layer and threads, which a ported engine might need instead).
