//! Host tests of the shell console's lines (shell/src/ring.rs): wrapping, the input line's positions, truncation, the
//! scrollback, and a console that follows its window's size (211-APP-0040); what the shell takes from a client of its
//! commands (shell/src/clients.rs, 211-APP-0044); `>` and `>>` (shell/src/redirect.rs, 000-APP-0074); the lines on
//! the way to the first frame (shell/src/start.rs, 000-APP-0075); a safe start told from the boot's log
//! (shell/src/safe.rs, 000-APP-0086); `netstat`'s rows (shell/src/netstat.rs, 000-APP-0031); `ping`'s options and
//! tally (shell/src/ping.rs, 000-APP-0095).
#![allow(dead_code)]
extern crate alloc;
#[path = "../shell/src/ring.rs"]
mod ring;
#[path = "../shell/src/redirect.rs"]
mod redirect;
#[path = "../shell/src/clients.rs"]
mod clients;
#[path = "../shell/src/start.rs"]
mod start;
#[path = "../shell/src/safe.rs"]
mod safe;
#[path = "../shell/src/netstat.rs"]
mod netstat;
#[path = "../shell/src/ping.rs"]
mod ping;
use ring::{Position, Ring, LINES};

fn ring(stride: usize, cols: usize, rows: usize) -> Ring<Vec<u32>> { Ring::new(vec![0; LINES * stride], stride, cols, rows) }

fn write(ring: &mut Ring<Vec<u32>>, text: &str) { for ch in text.chars() { ring.put(ch); } }

// Line `line` as text, `width` characters of it, without trailing spaces.
fn line(ring: &mut Ring<Vec<u32>>, line: u64, width: usize) -> String {
    ring.row(line)[..width].iter().map(|&c| char::from_u32(c).unwrap()).collect::<String>().trim_end().to_string()
}

// The lines on view, each `width` characters.
fn view(ring: &mut Ring<Vec<u32>>, width: usize) -> Vec<String> {
    let (top, bottom) = ring.view();
    (top..=bottom).map(|l| line(ring, l, width)).collect()
}

#[test]
fn lines_wrap_at_the_width_and_the_input_line_is_found() {
    let mut r = ring(10, 10, 4);
    write(&mut r, "MIND> ");
    let prompt = r.position();
    assert_eq!(prompt, Position { line: 0, col: 6 });
    write(&mut r, "abcdefgh");
    assert_eq!(view(&mut r, 10), ["MIND> abcd", "efgh"]);
    // The cursor after 4 characters sits at the end of the full line; after 5 on the next one.
    assert_eq!(r.offset(prompt, 4), Position { line: 0, col: 10 });
    assert_eq!(r.offset(prompt, 5), Position { line: 1, col: 1 });
    // The input line redrawn: everything after the prompt goes.
    r.truncate(prompt);
    assert_eq!(view(&mut r, 10), ["MIND>"]);
    assert_eq!(r.position(), prompt);
    // \r: what comes next overwrites the line; a step back clears a character.
    write(&mut r, "\nfirst\rFI\x08");
    assert_eq!(line(&mut r, 1, 10), "F rst");
}

#[test]
fn the_scrollback_keeps_its_lines_and_scrolls_by_pages() {
    let mut r = ring(8, 8, 5);
    for n in 0..LINES + 20 { write(&mut r, &format!("{}\n", n)); }
    // The newest lines on view, the last one being written; the oldest dropped.
    assert_eq!(view(&mut r, 8), [(LINES + 16).to_string(), (LINES + 17).to_string(), (LINES + 18).to_string(), (LINES + 19).to_string(), String::new()]);
    r.scroll(true);
    assert_eq!(r.back(), 4);
    for _ in 0..200 { r.scroll(true); }
    assert_eq!(r.back(), LINES - 5, "no further back than the oldest line kept");
    assert_eq!(view(&mut r, 8)[0], "21");
    assert!(r.unscroll() && !r.unscroll());
    r.clear();
    assert_eq!(view(&mut r, 8), [""]);
}

#[test]
fn a_console_in_a_window_follows_its_size() {
    // Room for 20 columns; the window starts 12 wide and 3 high.
    let mut r = ring(20, 12, 3);
    write(&mut r, "0123456789AB\nline two\nMIND> ");
    assert_eq!(view(&mut r, 12), ["0123456789AB", "line two", "MIND>"]);
    // Narrower: what was written keeps its characters; new text wraps at the new width.
    r.resize(6, 3);
    assert_eq!(view(&mut r, 6), ["012345", "line t", "MIND>"]);
    write(&mut r, "abcdef");
    assert_eq!(view(&mut r, 6), ["line t", "MIND>", "abcdef"]);
    // Wider and higher: the first line whole again, and more lines on view.
    r.resize(16, 6);
    assert_eq!(view(&mut r, 16), ["0123456789AB", "line two", "MIND>", "abcdef"]);
    write(&mut r, "ghijklmnopqrstuv");
    assert_eq!(view(&mut r, 16)[3..], ["abcdefghijklmnop", "qrstuv"]);
    // Never wider than its room; a new size shows the newest lines.
    r.scroll(true);
    r.resize(40, 2);
    assert_eq!((r.cols, r.rows, r.back()), (20, 2, 0));
}

#[test]
fn a_client_gets_the_observing_commands_at_once_and_asks_for_the_others() {
    use clients::{taken, Taken};
    // At once: what system information gives any program that asks for it.
    for line in ["ps", " QUOTAS ", "free", "ip", "netgrants", "date", "netpolicy", "sync", "endpoints"] { assert_eq!(taken(line), Taken::Now, "{}", line); }
    // What a command or program does (000-APP-0054): text only.
    for line in ["help kill", "help", "HELP fm"] { assert_eq!(taken(line), Taken::Now, "{}", line); }
    // In the shell's window once the user agrees there: what changes the machine or reaches beyond it, and a script by its
    // file (fm's Enter, 000-APP-0067).
    for line in ["kill 7", "reboot", "reboot -f", "poweroff", "shutdown -f", "stop", "budget 7 5 10", "netrevoke fetch", "netpolicy add allow x", "date set 2026-10-10 12:00",
                 "logs 7", "stat caps 7", "pmap 7", "caps 3", "logger hello", "ping ya.ru", "nslookup ya.ru", "fetch ya.ru /", "https ya.ru",
                 "msh /diag/quick.msh", "msh ram:x.msh"] { assert_eq!(taken(line), Taken::Asked, "{}", line); }
    // The screen's own commands, code typed for msh, programs and statements are refused.
    for line in ["fg 1", "boot", "keymap ru", "screenshot", "voice on", "msh -c \"print(1)\"", "msh --check x.msh", "msh x.msh a", "msh", "run fm", "fm", "caps", "let x = 1", "clear", "", "kill7", "date 1"] { assert_eq!(taken(line), Taken::Refused, "{:?}", line); }
}

#[test]
fn a_line_splits_at_its_redirection() {
    // 000-APP-0074: `>` and `>>` after a command, one file at the end; `>` inside double quotes is text.
    use redirect::{split, Split};
    assert_eq!(split("ls data > ram:l.txt"), Ok(Some(Split { command: "ls data", append: false, file: "ram:l.txt" })));
    assert_eq!(split("dmesg -s battery >>data/battery.txt"), Ok(Some(Split { command: "dmesg -s battery", append: true, file: "data/battery.txt" })));
    assert_eq!(split("ls>x"), Ok(Some(Split { command: "ls", append: false, file: "x" })));
    assert_eq!(split("ls data"), Ok(None));
    assert_eq!(split(r#"msh -c "print(2 > 1)""#), Ok(None));
    assert_eq!(split(r#"msh -c "print(2 > 1)" > ram:out.txt"#), Ok(Some(Split { command: r#"msh -c "print(2 > 1)""#, append: false, file: "ram:out.txt" })));
    assert!(split("> ram:x").is_err(), "no command");
    assert!(split("ls >").is_err(), "no file");
    assert!(split("ls > a b").is_err(), "two words after >");
    assert!(split("ls > a > b").is_err(), "two redirections");
    assert!(split("ls >>> a").is_err());
    assert!(split("run app & > ram:x").is_err(), "a background command");
    assert!(split("run app > ram:x &").is_err(), "& after the file");
}

#[test]
fn output_lines_go_to_the_file_and_errors_to_the_screen() {
    use redirect::{Lines, LONG};
    let (mut lines, mut file, mut shown) = (Lines::default(), Vec::new(), Vec::new());
    // A line comes whole even when its bytes come in pieces; one that starts `ERROR: ` is shown.
    lines.feed(b"one\ntw", &mut file, &mut shown);
    assert_eq!((file.as_slice(), shown.as_slice()), (&b"one\n"[..], &b""[..]));
    lines.feed(b"o\nERROR: CAT: NOT FOUND\nthree", &mut file, &mut shown);
    assert_eq!((file.as_slice(), shown.as_slice()), (&b"one\ntwo\n"[..], &b"ERROR: CAT: NOT FOUND\n"[..]));
    lines.finish(&mut file, &mut shown);
    assert_eq!(file.as_slice(), &b"one\ntwo\nthree"[..], "the last line without its end");
    // A long line without an end goes into the file in pieces.
    let (mut lines, mut file, mut shown) = (Lines::default(), Vec::new(), Vec::new());
    lines.feed(&vec![b'x'; LONG + 10], &mut file, &mut shown);
    assert_eq!(file.len(), LONG);
    lines.finish(&mut file, &mut shown);
    assert_eq!((file.len(), shown.len()), (LONG + 10, 0));
}

#[test]
fn a_client_is_asked_before_its_output_goes_into_a_file() {
    // 000-APP-0074: `>` writes a file in the shell's name, so a command taken at once is asked; a refused one stays so.
    use clients::{taken, Taken};
    assert_eq!(taken("ps"), Taken::Now);
    assert_eq!(taken("ps > data/ps.txt"), Taken::Asked);
    assert_eq!(taken("logs 7 >> ram:logs.txt"), Taken::Asked);
    assert_eq!(taken("ls > ram:x"), Taken::Refused, "ls is console's own");
    assert_eq!(taken("ps > a b"), Taken::Refused, "a line that is wrong");
}

#[test]
fn the_first_frame_and_a_slow_step_before_it_are_logged_once() {
    let mut start = start::Start::new(1_000);
    assert_eq!(start.step("THE FOCUS (THE KERNEL)", 1_200), None);
    assert_eq!(start.step("THE BOOT'S LOG (LOGD)", 1_200 + start::SLOW_MS), None, "exactly 5 s is not slow");
    assert_eq!(start.step("THE SERIAL LINE", 7_300), None, "1.1 s since the last step");
    let mut start = start::Start::new(500);
    assert_eq!(start.step("THE BOOT'S LOG (LOGD)", 6_501).as_deref(), Some("[SHELL] THE BOOT'S LOG (LOGD) TOOK 6001 MS BEFORE THE FIRST FRAME"));
    assert_eq!(start.step("THE SERIAL LINE", 7_000), None, "each step is timed from the last one");
    assert!(!start.drawn());
    assert_eq!(start.frame(2560, 1440, 7_949).as_deref(), Some("[SHELL] FIRST FRAME 2560X1440 AT 7.9 S"));
    assert!(start.drawn());
    assert_eq!(start.frame(2560, 1440, 8_000), None, "one line for the first frame");
    assert_eq!(start.step("THE SERIAL LINE", 60_000), None, "nothing after the first frame");
    let mut start = start::Start::new(0);
    start.stop();
    assert_eq!(start.step("THE SERIAL LINE", 60_000), None, "nothing is timed without a screen");
    assert_eq!(start.frame(640, 480, 60_000), None);
}

#[test]
fn a_safe_start_is_told_with_what_it_left_out() {
    let mut safe = safe::Safe::default();
    for line in ["[INIT] STARTED logd PID=2",
                 "[INIT] SAFE START: THE LAST RUN DID NOT END AS ASKED (A FREEZE, A RESET OR A CUT POWER); NOT STARTED: bcm_wifi, video_gw, audio_gw, tts, rtw89_wifi",
                 "[INIT] audio_gw LEFT OUT BY THE SAFE START, PREPARED: `svc start audio_gw` STARTS IT",
                 "[INIT] tts LEFT OUT BY THE SAFE START, PREPARED: `svc start tts` STARTS IT",
                 "[INIT] bcm_wifi NOT STARTED: NO DEVICE",
                 "[INIT] video_gw LEFT OUT BY THE SAFE START, NOT PREPARED: NoMemory",
                 "[INIT] tts LEFT OUT BY THE SAFE START, PREPARED: `svc start tts` STARTS IT"] {
        safe.line(line);
    }
    assert_eq!(safe.left_out, ["audio_gw", "tts"]);
    assert_eq!(safe.notice().unwrap(), "SAFE START: THE LAST RUN DID NOT END AS ASKED. LEFT OUT: audio_gw, tts (svc start audio_gw, svc start tts)");
    assert!(safe.notice().unwrap().len() <= 240);
}

#[test]
fn a_normal_boot_tells_nothing() {
    let mut safe = safe::Safe::default();
    // Only init's line counts: the shell's own copy of the notice in the log starts none.
    for line in ["[INIT] STARTED tts PID=21", "[SHELL] SAFE START: THE LAST RUN DID NOT END AS ASKED", "[INIT] READY"] { safe.line(line); }
    assert_eq!(safe.notice(), None);
}

#[test]
fn netstat_rows_line_up_under_the_header() {
    let row = netstat::Row { protocol: "TCP", local_port: 49153, remote: Some((0x0A00_0202, 8080)), state: netstat::state(5), sent: 120, received: 53_400, program: "download", pid: 23 };
    let line = netstat::line(&row);
    assert_eq!(line, "TCP   49153 10.0.2.2:8080         ESTABLISHED  120        53400      download (PID 23)");
    for word in ["LOCAL", "REMOTE", "STATE", "SENT", "RECEIVED", "PROGRAM"] {
        let at = netstat::HEADER.find(word).unwrap();
        assert_ne!(line.as_bytes()[at], b' ', "{} starts a column", word);
        assert_eq!(line.as_bytes()[at - 1], b' ', "{} has a space before it", word);
    }
    let udp = netstat::Row { protocol: "UDP", local_port: 68, remote: Some((0, 0)), state: netstat::state(0), sent: 0, received: 0, program: "netstack", pid: 9 };
    assert_eq!(netstat::line(&udp), "UDP   68    -                     -            0          0          netstack (PID 9)");
    assert_eq!((netstat::state(11), netstat::state(12)), ("TIME-WAIT", "?"));
}

#[test]
fn ping_takes_its_count_interval_wait_and_fast_mode() {
    let plain = ping::parse("10.0.2.2").unwrap();
    assert_eq!(plain, ping::Asked { host: "10.0.2.2", count: 3, interval_ms: 1000, wait_ms: 1000, fast: false });
    let asked = ping::parse("-c 5 -i 0.2 -w 1.5 ya.ru").unwrap();
    assert_eq!((asked.host, asked.count, asked.interval_ms, asked.wait_ms), ("ya.ru", 5, 200, 1500));
    assert_eq!(ping::parse("10.0.2.2 -C 0 -I .05").map(|a| (a.count, a.interval_ms)), Ok((0, 50)));
    // -f: the next request at once and 100 of them, unless -c or -i says otherwise.
    assert_eq!(ping::parse("-f 10.0.2.2").map(|a| (a.count, a.interval_ms, a.fast)), Ok((100, 0, true)));
    assert_eq!(ping::parse("-f -c 20 -i 0.01 10.0.2.2").map(|a| (a.count, a.interval_ms)), Ok((20, 10)));
    for wrong in ["", "-c", "-c x 10.0.2.2", "-c 1000001 h", "-i 61 h", "-i 0.0001 h", "-i 1.2.3 h", "-i . h", "-w 0.05 h", "-w 31 h",
                  "-x h", "a b"] {
        assert!(ping::parse(wrong).is_err(), "{:?}", wrong);
    }
    assert!(ping::parse("-s 64 h").unwrap_err().contains("NETWORK STACK"));
    assert!(ping::parse("-p ff h").unwrap_err().contains("NETWORK STACK"));
}

#[test]
fn ping_ends_with_its_tally() {
    let line = |tally: &ping::Tally, stopped| { let mut out = String::new(); tally.line(&mut out, stopped); out };
    let mut tally = ping::Tally::default();
    assert_eq!(line(&tally, false), "PING: 0 SENT, 0 RECEIVED, 0% LOST\n");
    for us in [800, 120, 100] { tally.reply(us); }
    // The suites read `PING: 3 SENT, 3 RECEIVED` at its start.
    assert_eq!(line(&tally, false), "PING: 3 SENT, 3 RECEIVED, 0% LOST, TIME 100/340/800 US (LEAST/MEAN/MOST)\n");
    tally.lost();
    assert_eq!(line(&tally, true), "PING: 4 SENT, 3 RECEIVED, 25% LOST, TIME 100/340/800 US (LEAST/MEAN/MOST), STOPPED\n");
    let mut lost = ping::Tally::default();
    for _ in 0..3 { lost.lost(); }
    assert_eq!(line(&lost, false), "PING: 3 SENT, 0 RECEIVED, 100% LOST\n");
}
