//! The screens in words, without system calls (tests/shell_host.rs, 213-APP-0158): `screen [second [keep|stretch]]`,
//! how the compositor shows the first screen on the second (display.wit 1.4's fit), and data/displays.txt, which keeps
//! the choice for the shell to set again at its start, as the compositor forgets it when it restarts.
use alloc::format;
use alloc::string::String;

/// The file, the one 177's layout of the screens is planned in.
pub const FILE: &str = "data/displays.txt";

/// `display.wit` 1.4's `fit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit { Keep, Stretch }

impl Fit {
    pub fn name(self) -> &'static str { match self { Fit::Keep => "keep", Fit::Stretch => "stretch" } }
    pub fn from_name(word: &str) -> Option<Fit> { match word { "keep" => Some(Fit::Keep), "stretch" => Some(Fit::Stretch), _ => None } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked { Show, Fit(Fit) }

pub const USAGE: &str = "SCREEN: SCREEN SECOND SHOWS THE SECOND SCREEN; SCREEN SECOND KEEP|STRETCH: THE FIRST SHOWN ON IT IN ITS OWN SHAPE OR FILLING IT, KEPT IN data/displays.txt";

/// `screen`, `screen second`, `screen second keep|stretch`.
pub fn parse(args: &str) -> Result<Asked, &'static str> {
    let lower = args.to_ascii_lowercase();
    let words: alloc::vec::Vec<&str> = lower.split_whitespace().collect();
    match words.as_slice() {
        [] | ["second"] => Ok(Asked::Show),
        ["second", fit] => Fit::from_name(fit).map(Asked::Fit).ok_or(USAGE),
        _ => Err(USAGE),
    }
}

/// The file's text for `fit`.
pub fn text(fit: Fit) -> String {
    format!("# The screens (177): how the first is shown on the second, keep (its own shape) or stretch (filling it); screen second sets it\nsecond {}\n", fit.name())
}

/// The most of the file the shell reads, and so rewrites.
pub const MAX: usize = 8192;

/// The file's text with the fit set to `fit`: the `second keep|stretch` lines `read` takes made one, or one added; every
/// other line kept as it was, the layout's and other tracks' (177, 213-DRV-0099). An empty file becomes `text`.
pub fn updated(old: &str, fit: Fit) -> String {
    if old.trim().is_empty() { return text(fit); }
    let mut out = String::new();
    let mut set = false;
    for line in old.lines() {
        let words: alloc::vec::Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
        if matches!(words.as_slice(), ["second", word] if Fit::from_name(word).is_some()) {
            if !set { out.push_str(&format!("second {}\n", fit.name())); set = true; }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !set { out.push_str(&format!("second {}\n", fit.name())); }
    out
}

/// The fit the file keeps; None when it names none. A line it does not know is left for the layout to come (177).
pub fn read(text: &str) -> Option<Fit> {
    text.lines().filter_map(|line| {
        let words: alloc::vec::Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
        match words.as_slice() { ["second", fit] => Fit::from_name(fit), _ => None }
    }).last()
}

/// The second screen as `mirror` gives it, or None while no monitor is offered.
pub fn second(mirror: Option<(u16, u16, Fit, u16, u16)>) -> String {
    match mirror {
        None => String::from("SCREEN: NO SECOND SCREEN: NO DISPLAY DRIVER OFFERS ONE"),
        Some((w, h, Fit::Keep, pw, ph)) => format!("SCREEN: THE SECOND SCREEN {w}X{h}, KEEP: THE FIRST SHOWN IN ITS OWN SHAPE AT {pw}X{ph}, CENTRED"),
        Some((w, h, Fit::Stretch, _, _)) => format!("SCREEN: THE SECOND SCREEN {w}X{h}, STRETCH: THE FIRST FILLS IT"),
    }
}

/// The line after a choice: what is now, and whether the file keeps it.
pub fn chosen(fit: Fit, kept: Result<(), &str>) -> String {
    let how = match fit { Fit::Keep => "IN ITS OWN SHAPE", Fit::Stretch => "FILLING IT" };
    match kept {
        Ok(()) => format!("SCREEN: THE SECOND SCREEN SHOWS THE FIRST {how} (SECOND {}), KEPT IN {FILE}", fit.name().to_ascii_uppercase()),
        Err(why) => format!("SCREEN: THE SECOND SCREEN SHOWS THE FIRST {how} (SECOND {}), NOT KEPT: {FILE}: {why}", fit.name().to_ascii_uppercase()),
    }
}
