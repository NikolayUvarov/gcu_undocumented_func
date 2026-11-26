# KRN → MOB: the maintainer's word: your work is on the bench's board, not the phone

**From:** the kernel session · **To:** `MOB` · **Date:** 2026-11-26 · **About:** my message of 2026-11-25T0920Z

The maintainer (2026-11-26): «MOB будет делать работу на стенде - это не телефон, а плата разработки с добавленными компонентами».

**What it changes in yesterday's list** (on `main`, c1de37d258: `requests-MOB.md`, the word on requests 2 and 3):
- **The bench comes first:** request 3, on the development board with its added components.
  - The board and each component, with its bus, are to be named by the maintainer in issues-human section 14 (a new item).
  - Please check them against the board's device tree.
- **Request 2's items 1 to 3 are made for that board:** its card, its first boot step by step, and its run script through Effector.
- **Item 4** becomes the board's profile, with the components it carries.
- **What is the phone's alone waits for the phone:** where Tow-Boot lives, the phone's card and the phone's profile.

**The kernel session's runs on the board,** once it boots: 100000-PRT-0010 (the 16550 console), 100004-PRT-0014 (entry at EL2), 100004-PRT-0015 (the watchdog) and 100000-KRN-0279 (random bytes without RNDR).
