#![no_std]
#![no_main]
// dmesg [-f] [-l debug|info|warn|error] [-s <name or PID>] [-n <count>]: the system log from logd (idl/log.wit), one
// record a line: seconds since boot, the source as logd stamped it (task name and PID), the text. -f keeps
// following new records (Esc stops it), -l hides records below a level, -s keeps one source, -n shows the last
// records only. A console program: it asks the shell for the log (the read badge).
use mind::abi::BootInfo;

mind::request!(REQUEST_CONSOLE | REQUEST_LOG);

struct Options<'a> { follow: bool, level: u8, source: Option<&'a str>, last: Option<u64> }

fn parse(args: &str) -> Result<Options<'_>, &'static str> {
    let mut options = Options { follow: false, level: 0, source: None, last: None };
    let mut words = args.split_whitespace();
    while let Some(word) = words.next() {
        match word {
            "-f" => options.follow = true,
            "-l" => {
                let word = words.next().unwrap_or("");
                let names: [&[&str]; 4] = [&["debug", "d", "0"], &["info", "i", "1"], &["warn", "warning", "w", "2"], &["error", "e", "3"]];
                options.level = names.iter().position(|n| n.iter().any(|n| n.eq_ignore_ascii_case(word))).ok_or("-l takes debug, info, warn or error")? as u8;
            }
            "-s" => options.source = Some(words.next().ok_or("-s takes a task name or a PID")?),
            "-n" => options.last = Some(words.next().and_then(|w| w.parse().ok()).ok_or("-n takes a count")?),
            _ => return Err("usage: dmesg [-f] [-l level] [-s name|pid] [-n count]"),
        }
    }
    Ok(options)
}

fn wanted(options: &Options, entry: &mind::log::Entry) -> bool {
    entry.level >= options.level && options.source.is_none_or(|s| s.eq_ignore_ascii_case(entry.name.as_str()) || s.parse::<u64>().is_ok_and(|pid| pid == entry.pid))
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("dmesg — the system log: seconds since boot, the source (task and PID), the text.\nUsage: dmesg [-f] [-l debug|info|warn|error] [-s <name or PID>] [-n <count>]\n-f follows new records (Esc stops), -l hides records below a level, -s keeps one source, -n the last records only.");
    mind::log::keep_output_local();
    let options = match parse(mind::process::args_str()) { Ok(o) => o, Err(text) => { mind::println!("dmesg: {}", text); return; } };
    let state = match mind::log::state() {
        Ok(state) => state,
        Err(mind::Error::NotFound) => { mind::println!("dmesg: no system log here (start dmesg from the shell)"); return; }
        Err(error) => { mind::println!("dmesg: the system log refused: {:?}", error); return; }
    };
    let mut from = options.last.map_or(0, |n| state.next.saturating_sub(n));
    if from < state.first { mind::println!("-- {} earlier records were dropped to make room --", state.first - from); from = state.first; }
    loop {
        let mut next = from;
        let got = mind::log::read(from, |entry| {
            if entry.seq > next { mind::println!("-- {} records lost --", entry.seq - next); }
            next = entry.seq + 1;
            if !wanted(&options, entry) { return; }
            let mark = match entry.level { 0 => "debug: ", 2 => "warning: ", 3 => "error: ", _ => "" };
            mind::println!("[{:>5}.{:03}] {}({}) {}{}", entry.time_ms / 1000, entry.time_ms % 1000, entry.name, entry.pid, mark, entry.text);
        });
        match got {
            Ok(0) if !options.follow => break,
            Ok(0) => { mind::time::sleep(250); }
            Ok(_) => from = next,
            Err(error) => { mind::println!("dmesg: {:?}", error); break; }
        }
    }
    if let Ok(state) = mind::log::state() {
        if state.suppressed > 0 { mind::println!("-- {} records were refused: their senders wrote more than 64 a second --", state.suppressed); }
    }
}
