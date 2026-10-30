//! Keys for INPUT (800-LNK-0032). A chord in QEMU's `sendkey` names (`ctrl-alt-f6`, `alt-0`, `shift-a`) is pressed in
//! order and released in reverse as PS/2 set 1 scan codes, which MIND Core's own decoder (`libmind/src/keys.rs`) turns
//! into input words: a modifier's press and release each a word, another key's press a word with its scan code as the
//! legacy byte, its release none. So a key over the link is the event `sendkey` gives. The decoder keeps the US layout
//! and never switches it: the machine's own layout is its keyboard driver's. Text is typed as the same chords; a
//! character no US key gives goes as a bare KEY_CHAR word, as a terminal's would.
use crate::abi::{input_event, KEY_CHAR};
use crate::ps2::{Event, Ps2, Switch};

/// The word INPUT carries for the attention key (Ctrl+Z): the focus goes back to the shell.
pub const ATTENTION: u64 = 0;

/// QEMU's key names and their PS/2 set 1 make codes; `true`: E0-prefixed.
const NAMES: &[(&str, u8, bool)] = &[
    ("esc", 0x01, false), ("1", 0x02, false), ("2", 0x03, false), ("3", 0x04, false), ("4", 0x05, false), ("5", 0x06, false),
    ("6", 0x07, false), ("7", 0x08, false), ("8", 0x09, false), ("9", 0x0A, false), ("0", 0x0B, false), ("minus", 0x0C, false),
    ("equal", 0x0D, false), ("backspace", 0x0E, false), ("tab", 0x0F, false),
    ("q", 0x10, false), ("w", 0x11, false), ("e", 0x12, false), ("r", 0x13, false), ("t", 0x14, false), ("y", 0x15, false),
    ("u", 0x16, false), ("i", 0x17, false), ("o", 0x18, false), ("p", 0x19, false), ("bracket_left", 0x1A, false),
    ("bracket_right", 0x1B, false), ("ret", 0x1C, false), ("ctrl", 0x1D, false),
    ("a", 0x1E, false), ("s", 0x1F, false), ("d", 0x20, false), ("f", 0x21, false), ("g", 0x22, false), ("h", 0x23, false),
    ("j", 0x24, false), ("k", 0x25, false), ("l", 0x26, false), ("semicolon", 0x27, false), ("apostrophe", 0x28, false),
    ("grave_accent", 0x29, false), ("shift", 0x2A, false), ("backslash", 0x2B, false),
    ("z", 0x2C, false), ("x", 0x2D, false), ("c", 0x2E, false), ("v", 0x2F, false), ("b", 0x30, false), ("n", 0x31, false),
    ("m", 0x32, false), ("comma", 0x33, false), ("dot", 0x34, false), ("slash", 0x35, false), ("shift_r", 0x36, false),
    ("kp_multiply", 0x37, false), ("asterisk", 0x37, false), ("alt", 0x38, false), ("spc", 0x39, false), ("caps_lock", 0x3A, false),
    ("f1", 0x3B, false), ("f2", 0x3C, false), ("f3", 0x3D, false), ("f4", 0x3E, false), ("f5", 0x3F, false), ("f6", 0x40, false),
    ("f7", 0x41, false), ("f8", 0x42, false), ("f9", 0x43, false), ("f10", 0x44, false), ("num_lock", 0x45, false),
    ("scroll_lock", 0x46, false), ("kp_7", 0x47, false), ("kp_8", 0x48, false), ("kp_9", 0x49, false), ("kp_subtract", 0x4A, false),
    ("kp_4", 0x4B, false), ("kp_5", 0x4C, false), ("kp_6", 0x4D, false), ("kp_add", 0x4E, false), ("kp_1", 0x4F, false),
    ("kp_2", 0x50, false), ("kp_3", 0x51, false), ("kp_0", 0x52, false), ("kp_decimal", 0x53, false), ("less", 0x56, false),
    ("f11", 0x57, false), ("f12", 0x58, false),
    ("kp_enter", 0x1C, true), ("ctrl_r", 0x1D, true), ("kp_divide", 0x35, true), ("print", 0x37, true), ("sysrq", 0x37, true),
    ("alt_r", 0x38, true), ("altgr", 0x38, true), ("home", 0x47, true), ("up", 0x48, true), ("pgup", 0x49, true),
    ("left", 0x4B, true), ("right", 0x4D, true), ("end", 0x4F, true), ("down", 0x50, true), ("pgdn", 0x51, true),
    ("insert", 0x52, true), ("delete", 0x53, true), ("meta_l", 0x5B, true), ("meta_r", 0x5C, true), ("menu", 0x5D, true),
];

/// The US keys of the printable ASCII characters typed without Shift, then with it.
const PLAIN: &[(char, &str)] = &[
    (' ', "spc"), ('\n', "ret"), ('\t', "tab"), ('-', "minus"), ('=', "equal"), ('[', "bracket_left"), (']', "bracket_right"),
    (';', "semicolon"), ('\'', "apostrophe"), ('`', "grave_accent"), ('\\', "backslash"), (',', "comma"), ('.', "dot"), ('/', "slash"),
];
const SHIFTED: &[(char, &str)] = &[
    ('!', "1"), ('@', "2"), ('#', "3"), ('$', "4"), ('%', "5"), ('^', "6"), ('&', "7"), ('*', "8"), ('(', "9"), (')', "0"),
    ('_', "minus"), ('+', "equal"), ('{', "bracket_left"), ('}', "bracket_right"), (':', "semicolon"), ('"', "apostrophe"),
    ('~', "grave_accent"), ('|', "backslash"), ('<', "comma"), ('>', "dot"), ('?', "slash"),
];

fn scan(name: &str) -> Option<(u8, bool)> {
    let name = name.to_ascii_lowercase();
    NAMES.iter().find(|(n, _, _)| *n == name).map(|&(_, code, extended)| (code, extended))
}

/// A keyboard on the master's side: its decoder's Caps Lock and modifiers last as long as it does.
pub struct Keyboard { decoder: Ps2 }

impl Default for Keyboard { fn default() -> Self { Self::new() } }

impl Keyboard {
    pub fn new() -> Self {
        let mut decoder = Ps2::new();
        decoder.set_switch(Switch::None);
        Self { decoder }
    }

    /// The words of one chord, `-`-separated QEMU names pressed in order and released in reverse.
    pub fn chord(&mut self, chord: &str) -> Result<Vec<u64>, String> {
        let keys = chord.split('-').map(|n| scan(n).ok_or_else(|| format!("{n:?} is not a key name (QEMU's sendkey names: ctrl, alt, shift, a, 1, f6, ret, spc, up, …)")))
            .collect::<Result<Vec<_>, _>>()?;
        if chord.is_empty() || keys.is_empty() { return Err("an empty chord".into()); }
        let mut bytes = Vec::new();
        for &(code, extended) in &keys { if extended { bytes.push(0xE0); } bytes.push(code); }
        for &(code, extended) in keys.iter().rev() { if extended { bytes.push(0xE0); } bytes.push(code | 0x80); }
        let mut words = Vec::new();
        for b in bytes {
            match self.decoder.feed(b) {
                Some(Event::Key(word)) => words.push(word as u64),
                Some(Event::Attention) => words.push(ATTENTION),
                Some(Event::Layout(_)) | None => {}
            }
        }
        Ok(words)
    }

    /// The words of each character of `text`, one group a character, as its US chord gives it; a character no US key
    /// gives is a KEY_CHAR press alone. A control character other than a new line and a tab is refused.
    pub fn text(&mut self, text: &str) -> Result<Vec<Vec<u64>>, String> {
        text.chars().filter(|&c| c != '\r').map(|c| {
            let chord = if c.is_ascii_lowercase() || c.is_ascii_digit() { c.to_string() }
                else if c.is_ascii_uppercase() { format!("shift-{}", c.to_ascii_lowercase()) }
                else if let Some((_, n)) = PLAIN.iter().find(|(p, _)| *p == c) { n.to_string() }
                else if let Some((_, n)) = SHIFTED.iter().find(|(p, _)| *p == c) { format!("shift-{n}") }
                else if c.is_control() { return Err(format!("U+{:04X}: a control character is typed as a key (key ret, key tab, key ctrl-c)", c as u32)) }
                else { return Ok(vec![input_event(0, KEY_CHAR, 0, true, c as u32) as u64]) };
            self.chord(&chord)
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::*;

    fn w(byte: u8, key: u16, mods: u8, pressed: bool, ch: char) -> u64 { input_event(byte, key, mods, pressed, ch as u32) as u64 }

    #[test]
    fn a_chord_is_the_events_ps2_gives() {
        let mut k = Keyboard::new();
        assert_eq!(k.chord("a").unwrap(), [w(0x1E, KEY_CHAR, 0, true, 'a')]);
        assert_eq!(k.chord("shift-a").unwrap(), [w(0, KEY_SHIFT, MOD_SHIFT, true, '\0'), w(0x1E, KEY_CHAR, MOD_SHIFT, true, 'A'), w(0, KEY_SHIFT, 0, false, '\0')]);
        assert_eq!(k.chord("ctrl-alt-f6").unwrap(), [
            w(0, KEY_CTRL, MOD_CTRL, true, '\0'), w(0, KEY_ALT, MOD_CTRL | MOD_ALT, true, '\0'), w(0x40, KEY_F1 + 5, MOD_CTRL | MOD_ALT, true, '\0'),
            w(0, KEY_ALT, MOD_CTRL, false, '\0'), w(0, KEY_CTRL, 0, false, '\0')]);
        assert_eq!(k.chord("alt-0").unwrap()[1], w(0x0B, KEY_CHAR, MOD_ALT, true, '0'));
        assert_eq!(k.chord("up").unwrap(), [w(0x48, KEY_UP, 0, true, '\0')]);
        assert_eq!(k.chord("ret").unwrap(), [w(0x1C, KEY_ENTER, 0, true, '\n')]);
        assert_eq!(k.chord("f12").unwrap(), [w(0x58, KEY_F1 + 11, 0, true, '\0')]);
        assert_eq!(k.chord("print").unwrap(), [w(0x37, KEY_PRINT_SCREEN, 0, true, '\0')]);
        // Ctrl+Z is the attention key, as ps2_kbd delivers it.
        assert_eq!(k.chord("ctrl-z").unwrap(), [w(0, KEY_CTRL, MOD_CTRL, true, '\0'), ATTENTION, w(0, KEY_CTRL, 0, false, '\0')]);
        // Ctrl+Shift switches no layout over the link.
        assert!(k.chord("ctrl-shift").unwrap().iter().all(|&e| matches!(event_key(e as usize), KEY_CTRL | KEY_SHIFT)));
        assert_eq!(k.chord("q").unwrap(), [w(0x10, KEY_CHAR, 0, true, 'q')]);
    }

    #[test]
    fn caps_lock_lasts_and_names_are_checked() {
        let mut k = Keyboard::new();
        assert!(k.chord("caps_lock").unwrap().is_empty());
        assert_eq!(event_char(k.chord("b").unwrap()[0] as usize), 'B' as u32);
        assert!(k.chord("ctrl-nope").unwrap_err().contains("\"nope\" is not a key name"));
        assert!(k.chord("").is_err());
        // Caps Lock is still on, so the event carries it, as PS/2's would.
        assert_eq!(k.chord("CTRL-C").unwrap()[1], w(0x2E, KEY_CHAR, MOD_CTRL | MOD_CAPS, true, 'c'));
    }

    #[test]
    fn text_is_typed_as_chords() {
        let mut k = Keyboard::new();
        let typed = k.text("Hi! ж\n").unwrap();
        assert_eq!(typed.len(), 6);
        assert_eq!(typed[0][1], w(0x23, KEY_CHAR, MOD_SHIFT, true, 'H'));
        assert_eq!(typed[1], [w(0x17, KEY_CHAR, 0, true, 'i')]);
        assert_eq!(typed[2][1], w(0x02, KEY_CHAR, MOD_SHIFT, true, '!'));
        assert_eq!(typed[3], [w(0x39, KEY_CHAR, 0, true, ' ')]);
        assert_eq!(typed[4], [w(0, KEY_CHAR, 0, true, 'ж')]);
        assert_eq!(typed[5], [w(0x1C, KEY_ENTER, 0, true, '\n')]);
        assert!(k.text("a\u{7}").unwrap_err().contains("U+0007"));
    }
}
