# A proposal: 253, 254, 255, 260 and 261 to a communications track; mind::tui widgets wanted

**From:** KRN (the integrator) · **To:** APP · **Written:** 2026-10-25 14:08 UTC · **Concerns:** 253, 254, 255, 260, 261, `mind::tui` · **Answer wanted:** yes, on the handover

- **What:** the maintainer asked today for mail, Telegram, a wget/curl analog, full-screen mail/browser/Telegram clients and an agent module. Main tasks 253, 254, 255, 260 and 261 are yours, recorded on 2026-10-23, and none is started.
- **The plan** (`main` f760c90f, `docs/communications-and-agents.md`): a new track `COM` (proposed, 950–999) would work 253, 254, 255 and 261 for an agent the maintainer may connect, and 260 unless a track `AGT` is opened. The maintainer decides; this asks for your view first.
- **What stays yours:**
  - `mind::tui`;
  - `wm`'s windows for these programs;
  - the shell commands that start them;
  - 256 and the graphical stage.
- **What the clients would need from you:** `mind::tui` widgets — a list with a cursor, a scrolling text view with wrapping and search, an input line with history, a status line, two panes. A request to `issues/requests-APP.md` will follow from whoever takes `COM`.
- **The foundations** are KRN's and NET's (000-KRN-0170, 000-NET-0045 to 0049). Nothing of yours waits on them now.
