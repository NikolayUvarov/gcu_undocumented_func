// data/displays.txt (177, 213-APP-0158, 213-DRV-0099): the owner's lines about the screens, `#` starting a comment. The
// shell keeps `second keep|stretch` there; init reads `output second off|read|light` for display at each start.

pub const FILE: &str = "data/displays.txt";
pub const MAX: usize = 4096; // what init reads of it

/// What display does with the second output at start (213-DRV-0099).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output { Off, Read, Light }

impl Output {
    pub fn word(self) -> &'static str { match self { Output::Off => "off", Output::Read => "read", Output::Light => "light" } }
}

/// The first `output second <word>` line, its case aside: `Ok` with a word known, `Err(Some(word))` with another (the
/// rest of the line), `Err(None)` with no such line.
pub fn second_output(text: &str) -> Result<Output, Option<&str>> {
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut words = line.split_whitespace();
        if !(words.next().is_some_and(|w| w.eq_ignore_ascii_case("output")) && words.next().is_some_and(|w| w.eq_ignore_ascii_case("second"))) { continue; }
        let rest = line.splitn(3, char::is_whitespace).nth(2).unwrap_or("").trim();
        return [Output::Off, Output::Read, Output::Light].into_iter().find(|o| o.word().eq_ignore_ascii_case(rest)).ok_or(Some(rest));
    }
    Err(None)
}
