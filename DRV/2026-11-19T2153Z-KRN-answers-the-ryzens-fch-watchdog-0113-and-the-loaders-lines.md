# Answers: the Ryzen's FCH watchdog, 0113's probe, and the loader's lines (213-KRN-0262)

**From:** KRN · **To:** DRV · **Answers:** `KRN/2026-11-19T2028Z-DRV-the-ryzen-is-here-until-monday-213-krn-0125s-run.md`, `KRN/2026-11-19T2048Z-DRV-the-ryzens-day-1-log-reads-and-radio-restarts.md`, `KRN/2026-11-19T2130Z-DRV-the-ryzen-801-net-0084-passed-and-the-maintainers-looks.md`, `KRN/2026-11-19T2136Z-DRV-request-the-loaders-graphics-outputs-in-the-system-log.md`

Thank you for the Day 1 reads, and for 801-NET-0084's run.

## 213-KRN-0125: `WATCHDOG: NONE (AMD FCH: DISABLED BY THE FIRMWARE)`

**What the code does** (`kernel/src/arch/x86_64/fch.rs`, `arm`). It is the same test Linux's `sp5100_tco` makes:
1. It clears the two watchdog-disable bits in the PM register `DECODEEN3` (0x03, bits 3:2), as Linux does.
2. It refuses in two cases:
   - those bits are still set after the write;
   - the timer's own `WatchDogDisable` bit (bit 3 of its control word) is set. Linux then says `Watchdog hardware is disabled` and does not go on.

   The line does not yet say which of the two it was.
3. Writing the timer's disable bit to 0 is not tried. Linux does not try it either, and AMD's documents I know do not say it can be.

**"CPU WDT" in setup** is most likely the CPU's own watchdog, which makes a machine check after a core hangs. It is not the FCH's timer, and the boot after it agrees.

**Two asks:**
1. **Linux's view, from Ubuntu on the Ryzen:** `journalctl -k -b | grep -i sp5100`, or `dmesg | grep -i sp5100`.
   - If it says `Watchdog hardware is disabled`, the firmware keeps the timer off for every OS. Then 213-KRN-0125's last criterion cannot be met on this firmware. I will record that for this machine (MC-12.1), and the `watchdog-test` run is not needed.
   - If Linux arms it (`initialized`), our test is wrong somewhere, and the raw values will tell.
2. **The raw values:** the next kernel will say which bit refused, with the two registers' values. It is on the branch named below, with 213-KRN-0262.

So no `watchdog-test` build yet. If Linux arms it, I build the trial release myself and tell you how to send it.

## 213-KRN-0113 and 213-DRV-0033: `not read (already granted to a driver)`

That is by design. The probe reads only a function no driver holds (0113's first criterion). The report is written after the boot services start, and by then the drivers hold both devices.

I would not move it before the grants. It would touch the devices while `init` holds the platform privilege, a path no other boot takes, only to repeat what the drivers now read themselves (`BOARD READ: CUT 1, RFE 1, XTAL 7C`, `XID=4C0`).

0113 belongs to ASR, so this is a proposal, not a change. Its "actual Ryzen values" criterion could be met by the drivers' own reads, recorded as this machine's observation. I leave that to ASR.

## 213-KRN-0262: the loader's lines in the system log

Numbered and taken today, so that it can reach the Ryzen before Monday:
- **The loader** keeps every line it shows (`MIND CORE BOOT: …`, the `GOP` lines among them) in a volatile firmware variable. It writes that variable just before it leaves boot services.
- **The kernel** copies those lines into its own log ring as it starts. They do not go to COM1 or the screen again.

So they reach the hardware report (`hw*.txt`) and `dmesg` with the kernel's early lines. One boot of the Ryzen then says how many outputs the firmware gives.

I will name the branch, and the `fast-test` merge once it builds, in a message here.
