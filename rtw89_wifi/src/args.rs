//! The driver's start arguments from `init`: the PCI device index, then 213-KRN-0122's platform word.

const REASON_MAX: usize = 48;

/// The platform's transmit power policy as `init` read it; anything but a well-formed word is `Unknown`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform<'a> {
    Absent,
    /// Half-dBm: the per-NTX limits, then the per-path SAR reference values.
    Limits { per_ntx: [i8; 2], per_path: [i8; 2] },
    /// Init's reason when it gave one in printable ASCII of at most 48 bytes.
    Unknown(Option<&'a str>),
}

fn limits(text: &str) -> Option<[i8; 4]> {
    let mut values = [0i8; 4];
    let mut fields = text.split(',');
    for value in &mut values {
        *value = fields.next()?.parse().ok().filter(|v| (-64..=63).contains(v))?;
    }
    fields.next().is_none().then_some(values)
}

fn platform(value: &str) -> Platform<'_> {
    if value == "absent" { return Platform::Absent; }
    if let Some([a, b, c, d]) = value.strip_prefix("limits:").and_then(limits) {
        return Platform::Limits { per_ntx: [a, b], per_path: [c, d] };
    }
    let reason = value.strip_prefix("unknown:")
        .filter(|r| !r.is_empty() && r.len() <= REASON_MAX && r.bytes().all(|b| (0x21..0x7f).contains(&b)));
    Platform::Unknown(reason)
}

/// The first word is the decimal index; a later `platform=` word is read once, other words are left to their owners.
/// A second `platform=` word makes the policy `Unknown`: two answers are no answer.
pub fn parse(args: &str) -> Option<(usize, Platform<'_>)> {
    let mut words = args.split_ascii_whitespace();
    let index = words.next()?.parse().ok()?;
    let mut found = None;
    for word in words {
        let Some(value) = word.strip_prefix("platform=") else { continue };
        found = match found { None => Some(platform(value)), Some(_) => Some(Platform::Unknown(None)) };
    }
    Some((index, found.unwrap_or(Platform::Unknown(None))))
}
