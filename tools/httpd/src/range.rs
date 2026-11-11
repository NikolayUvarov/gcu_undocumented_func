//! One byte range per request (RFC 9110, section 14), which the updater uses to resume a download (351-NET-0001).

/// What a `Range` header asks of a file of `len` bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// The whole file: no header, more than one range, or one the server ignores as RFC 9110 allows.
    Whole,
    /// Bytes `start..=end`.
    Part(u64, u64),
    /// None of the file (416).
    Unsatisfiable,
}

fn number(s: &str) -> Option<u64> { if s.is_empty() || s.len() > 19 || !s.bytes().all(|b| b.is_ascii_digit()) { None } else { s.parse().ok() } }

pub fn asked(header: Option<&str>, len: u64) -> Asked {
    let Some(value) = header else { return Asked::Whole };
    let Some(spec) = value.trim().strip_prefix("bytes=") else { return Asked::Whole };
    if spec.contains(',') { return Asked::Whole; }
    let Some((first, last)) = spec.trim().split_once('-') else { return Asked::Whole };
    match (first.is_empty(), number(first), number(last)) {
        // `-K`: the last K bytes.
        (true, _, Some(k)) => if k == 0 || len == 0 { Asked::Unsatisfiable } else { Asked::Part(len.saturating_sub(k), len - 1) },
        // `N-`: from N to the end.
        (false, Some(n), None) if last.is_empty() => if n < len { Asked::Part(n, len - 1) } else { Asked::Unsatisfiable },
        // `N-M`: N to M, M cut to the end.
        (false, Some(n), Some(m)) if n <= m => if n < len { Asked::Part(n, m.min(len - 1)) } else { Asked::Unsatisfiable },
        _ => Asked::Whole,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(asked(None, 100), Asked::Whole);
        assert_eq!(asked(Some("bytes=10-"), 100), Asked::Part(10, 99));
        assert_eq!(asked(Some("bytes=10-19"), 100), Asked::Part(10, 19));
        assert_eq!(asked(Some("bytes=90-200"), 100), Asked::Part(90, 99));
        assert_eq!(asked(Some("bytes=-30"), 100), Asked::Part(70, 99));
        assert_eq!(asked(Some("bytes=-300"), 100), Asked::Part(0, 99));
        assert_eq!(asked(Some("bytes=100-"), 100), Asked::Unsatisfiable);
        assert_eq!(asked(Some("bytes=150-160"), 100), Asked::Unsatisfiable);
        assert_eq!(asked(Some("bytes=-0"), 100), Asked::Unsatisfiable);
        assert_eq!(asked(Some("bytes=0-"), 0), Asked::Unsatisfiable);
        for ignored in ["bytes=0-1,5-6", "bytes=20-10", "bytes=a-", "bytes=-", "items=0-1", "bytes=1", "bytes=99999999999999999999-"] {
            assert_eq!(asked(Some(ignored), 100), Asked::Whole, "{ignored}");
        }
    }
}
