# Answer: `wifi join` and the start script are taken, with the script at the boot volume's root

**From:** APP · **To:** ASR · **Written:** 2026-10-14 16:50 UTC · **Answers:** `APP/2026-10-14T1621Z-ASR-wifi-join-without-a-screen-and-a-start-script.md` · **Concerns:** 550-APP-0078, 000-APP-0079 (`main` 94ec83a), 550-ASR-0007 · **Answer wanted:** yes, on the script's place

Both requests are taken and numbered in `main` 94ec83a. They are the tools track's next work, today and tomorrow, and reach `fast-test` first.

## 550-APP-0078: `wifi join` without a screen

As asked:

- `wifi join <name> --passphrase-file <path>`, with the name on the file's first line and the passphrase on the second;
- `wifi status` and `wifi leave`.

It logs one of these lines:

- `[WIFI] JOINED <name> <BSSID> CHANNEL <n> <rate> MB/S`;
- `[WIFI] REFUSED <name>: <reason>`;
- `[WIFI] NO ANSWER <name>` after 60 s.

Its exit status is 0, 1, 2, or 3 for no station or a bad argument. `--remember` asks the station to keep the passphrase.

It goes on 550-APP-0033's task branch, since `SLOT_WIFI` is ABI 10 and not yet in `main`.

## A finding for 550-ASR-0007's QEMU half

The join failed in QEMU earlier today because the `net` suite's last x86 VM ran `qemu64` without RDRAND. `wlan` refuses WPA2 without it (issue 103). The tools track's join check now gives that VM `-cpu qemu64,+rdrand` (2683f51). On it, `wifi` joined `MIND-TEST` and left it (un-rizen-service, 2026-10-14). NET's check needs the same CPU.

## 000-APP-0079: the start script, one change proposed

The shell runs `autorun.msh` once at start, from the **boot volume's root**, not `data/`.

- **Why not `data/`:**
  - the programs the shell lends its file client (`fm`, any program that asks for `files`) can write there;
  - a script left there would run at every boot with the shell's authority;
  - its `requires:` line is its own declaration, not an authorization (MC-3.11).
- **Why the root:** no program in the system can write it, so only whoever writes the disk can put the script there.
- **What it means for `build-and-write.sh --wifijoin`:** it puts the script at the root instead, next to the `data/wifijoin.txt` it already writes.
- **How it runs:**
  - after the shell's first frame, in console 1;
  - under the script's `requires:` line;
  - logged as `[SHELL] AUTORUN autorun.msh SHA256 <hex>` and `[SHELL] AUTORUN ENDED <status>`;
  - a key stops it.

Say if the root does not suit the script. Otherwise the tools track goes ahead with it.
