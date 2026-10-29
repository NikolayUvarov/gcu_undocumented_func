// svc's edits of data/services.txt (173-APP-0092): the file's other lines kept as they are, the directives a change
// replaces left out, its own appended; init's own parser (`services.rs`) judges the result, so svc refuses what init would.
use crate::services::{self, Plan, MAX};

/// What a command changes: a service on or off at the next boot, or started after another.
#[derive(Clone, Copy)]
pub enum Change<'a> { Enable(&'a str), Disable(&'a str), After(&'a str, &'a str) }

/// The file after a change: its text in `out[..len]`, init's plan of it, and whether it differs from the file before.
pub struct Edited<const N: usize> { pub len: usize, pub plan: Plan<N>, pub changed: bool }

/// The units in init's boot order: init, then each image followed by its further instances (as `boot_order` in init).
pub fn units<const N: usize>(images: &[&'static str], instances: &[&'static str]) -> [&'static str; N] {
    let mut units = ["init"; N];
    let mut p = 1;
    for image in images.iter().skip(1) {
        if p < N { units[p] = image; p += 1; }
        for instance in instances.iter().filter(|i| services::base(i) == *image) { if p < N { units[p] = instance; p += 1; } }
    }
    units
}

/// The plan of the file's text as init reads it; `refused(line, why, text)` for each line init leaves out.
pub fn read<const N: usize>(text: &[u8], units: &[&str; N], refused: impl FnMut(usize, &'static str, &str)) -> Plan<N> {
    services::parse(text, units, refused)
}

// A line's words as init reads them: up to `#`, split on white space; None when not ASCII.
fn words(line: &[u8]) -> Option<[Option<&str>; 4]> {
    let line = core::str::from_utf8(line).ok().filter(|l| l.is_ascii())?;
    let mut words = line.split('#').next().unwrap_or("").split_ascii_whitespace();
    Some([words.next(), words.next(), words.next(), words.next()])
}

// Whether `line` is a directive that `change` replaces: `enable` or `disable` of the same name, or `after` of it.
fn replaced(line: &[u8], change: Change) -> bool {
    match (words(line), change) {
        (Some([Some("enable" | "disable"), Some(name), None, None]), Change::Enable(n) | Change::Disable(n)) => name == n,
        (Some([Some("after"), Some(name), Some(_), None]), Change::After(n, _)) => name == n,
        _ => false,
    }
}

fn push(out: &mut [u8; MAX], len: &mut usize, bytes: &[u8]) -> Result<(), &'static str> {
    let end = *len + bytes.len();
    if end > MAX { return Err("THE FILE WOULD PASS 4 KiB, WHAT INIT READS"); }
    out[*len..end].copy_from_slice(bytes);
    *len = end;
    Ok(())
}

/// `text` (data/services.txt, empty if absent) with `change` made, in `out`; `refused` for each of the file's other lines
/// that init leaves out. Err: why init would refuse the change, or why svc cannot make it.
pub fn edit<const N: usize>(text: &[u8], units: &[&str; N], change: Change, out: &mut [u8; MAX], mut refused: impl FnMut(usize, &'static str, &str)) -> Result<Edited<N>, &'static str> {
    if text.len() > MAX { return Err("DATA/SERVICES.TXT IS LONGER THAN 4 KiB AND INIT READS ITS FIRST 4 KiB ONLY: svc reset REMOVES IT"); }
    let (name, other) = match change { Change::Enable(n) | Change::Disable(n) => (n, None), Change::After(n, o) => (n, Some(o)) };
    // `#` starts a comment for init: the image's name covers its instances.
    if name.contains('#') || other.is_some_and(|o| o.contains('#')) { return Err("AN INSTANCE (NAME#N) CANNOT BE NAMED: ITS IMAGE'S NAME COVERS EVERY INSTANCE"); }
    let mut line = [0u8; 64];
    let mut length = 0;
    match change {
        Change::Enable(n) => { push_line(&mut line, &mut length, &["enable ", n])?; }
        Change::Disable(n) => { push_line(&mut line, &mut length, &["disable ", n])?; }
        Change::After(n, o) => { push_line(&mut line, &mut length, &["after ", n, " ", o])?; }
    }
    let directive = &line[..length];
    // init's rules alone first: an unknown or essential name, an order against init's own.
    let mut why = None;
    services::parse(directive, units, |_, w, _| why = why.or(Some(w)));
    if let Some(why) = why { return Err(why); }
    // A file that has the directive already, and no other line it replaces, stays as it is.
    let lines = || text.split(|&b| b == b'\n').enumerate().filter(|(_, raw)| replaced(raw, change));
    if let (1, Some((i, raw))) = (lines().count(), lines().next()) {
        if words(raw) == words(directive) {
            let mut mine = None;
            let plan = services::parse(text, units, |l, w, t| if l == i + 1 { mine = Some(w) } else { refused(l, w, t) });
            if let Some(why) = mine { return Err(why); }
            out[..text.len()].copy_from_slice(text);
            return Ok(Edited { len: text.len(), plan, changed: false });
        }
    }
    // The file's lines but those the change replaces.
    let mut len = 0;
    for raw in text.split(|&b| b == b'\n') {
        if replaced(raw, change) { continue; }
        push(out, &mut len, raw)?;
        push(out, &mut len, b"\n")?;
    }
    // A text that did not end with a newline gained one; one that did gained an empty line.
    if text.last() == Some(&b'\n') || text.is_empty() { len -= 1; }
    let kept = services::parse(&out[..len], units, |_, _, _| {});
    let positions = || (0..N).filter(|&p| units[p] == name || services::base(units[p]) == name);
    // `enable` needs no line once nothing turns the service off: the file's default is on.
    let append = match change { Change::Enable(_) => positions().any(|p| kept.off[p]), _ => true };
    let mut ours = 0;
    if append {
        ours = out[..len].iter().filter(|&&b| b == b'\n').count() + 1;
        push(out, &mut len, directive)?;
        push(out, &mut len, b"\n")?;
    }
    let mut mine = None;
    let plan = services::parse(&out[..len], units, |l, w, t| if l == ours && append { mine = Some(w) } else { refused(l, w, t) });
    if let Some(why) = mine { return Err(why); }
    Ok(Edited { len, plan, changed: &out[..len] != text })
}

fn push_line(line: &mut [u8; 64], length: &mut usize, parts: &[&str]) -> Result<(), &'static str> {
    for part in parts {
        let end = *length + part.len();
        if end > line.len() { return Err("UNKNOWN SERVICE"); }
        line[*length..end].copy_from_slice(part.as_bytes());
        *length = end;
    }
    Ok(())
}
