# KRN ready: `get -u USER` with the password typed at the console, and the Wi-Fi checks' `dmesg` wait

**Ready:** `claude/youthful-mendel-mf1soy` 657680074d95

**From:** the kernel session (KRN, PRT, NET)

**What it lands** (on `main` 83376ccc):
- **ca1e9e50, 000-NET-0048 (advances; its last item):** `get -u USER` without a password has it typed at its console.
  - It goes through the shell's secret line (`REQUEST_SECRET_LINE`, `shell.wit` 1.12, APP's 000-APP-0126, in `main` since round 102).
  - The prompt names the user and the URL's host, the line is typed with no echo, and `get` wipes its copy once the header is built.
  - Esc, and a script's `get`, exit 2.
  - Also: `help get`, `docs/network/downloads.md`, and the `net` suite's `get_runs`.
- **1012596d, 000-NET-0063 (new):** round 102's one failure, `dmesg -s wifi` past its 8 s in the group "x86: RAM above 4 GiB" on x32-128.
  - The CPUs' state showed `dmesg` still decoding the log's records: it was slow beside the other lane's build, not stuck.
  - The eight `dmesg -s` reads of the Wi-Fi checks wait 30 s now. `dmesg` itself (APP's) does not change.
- The issues' progress (2e179e42, 65768007) and `main`'s text commits merged in.

**Tested:**
- **The `net` suite on x86:** passed twice in the kernel session's container (QEMU 8.2.2, TCG, 4 CPUs; 39 checks, none skipped). The second run had the 30 s wait.
  - `get_runs`' new part: `-u alice:s3:cr` as Basic authentication and not sent on to another port.
  - `-u alice` with a password typed at the serial console. It was sent, and appeared in no serial line and no line of the shell's log.
  - Esc refused it (2), and a script's `get -u alice` was refused (2).
- **The group "aarch64: network, TLS and Effector":** passed on acc-1 through Effector, at 1012596d (1008 s).
- **The host tests:** `get_host` (6).
- **Not run:** the other groups. The change touches `get`, the `net` suite's checks and text only.
