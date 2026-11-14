//! `effector policy` (801-APP-0140, the policy part of 750-APP-0093): the owner's Effector policy, `data/effector/policy`
//! (750-EFF-0003), shown and edited one rule at a time. The whole new file must parse before it is written, so an edit
//! never leaves a policy the gateway refuses, which would allow nothing. The parser is the caller's (`check`), the one
//! `effector_gw` and `init` use; this file is the command line and the text, host-tested in tests/shell_host.rs.
use alloc::string::String;

pub const FILE: &str = "data/effector/policy";
/// The new file, written whole and read back before it takes the old one's place.
pub const NEW: &str = "data/effector/policy.new";
/// The old file, kept until the new one has its name.
pub const OLD: &str = "data/effector/policy.old";
pub const DIR: &str = "data/effector";
/// The first line of a policy that is written new.
pub const HEADER: &str = "effector-policy 1";
pub const USAGE: &str = "USAGE: EFFECTOR POLICY [SHOW | ALLOW <LINE> | DENY <LINE>]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked<'a> { Show, Allow(&'a str), Deny(&'a str) }

/// `effector`'s words after the command: `policy [show | allow <line> | deny <line>]`.
pub fn parse(args: &str) -> Result<Asked<'_>, &'static str> {
    let args = args.trim();
    let (word, rest) = args.split_once(char::is_whitespace).map_or((args, ""), |(w, r)| (w, r.trim()));
    if !word.eq_ignore_ascii_case("policy") { return Err(USAGE); }
    let (verb, line) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(v, l)| (v, l.trim()));
    match verb.to_ascii_lowercase().as_str() {
        "" | "show" if line.is_empty() => Ok(Asked::Show),
        "allow" if !line.is_empty() => Ok(Asked::Allow(line)),
        "deny" if !line.is_empty() => Ok(Asked::Deny(line)),
        _ => Err(USAGE),
    }
}

/// What a rule is: the key, and for a key that may come several times the name or root it is about. Two lines with the
/// same identity are one rule, the newer in place of the older.
fn identity(line: &str) -> Option<(&str, &str)> {
    let mut words = line.split_whitespace();
    let key = words.next()?;
    if key.starts_with('#') { return None; }
    Some(match key {
        "service" | "package-key" | "collect" | "log" => (key, words.next().unwrap_or("")),
        _ => (key, ""),
    })
}

/// What an edit did, for the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change { Added(String), Replaced { old: String, new: String }, Removed(String) }

/// The policy after `asked` (`Allow` or `Deny`) on `current` (None: no file), or why not. The result is `check`ed whole:
/// `check` returns the rules' count, or the line it refused and why.
pub fn edit(current: Option<&str>, asked: Asked<'_>, check: impl Fn(&str) -> Result<u16, (u16, &'static str)>) -> Result<(String, Change, u16), String> {
    let (line, allow) = match asked { Asked::Allow(line) => (line, true), Asked::Deny(line) => (line, false), Asked::Show => return Err(String::from(USAGE)) };
    // One space between words, as the parser reads them.
    let mut rule = String::new();
    for word in line.split_whitespace() { if !rule.is_empty() { rule.push(' '); } rule.push_str(word); }
    let Some(wanted) = identity(&rule) else { return Err(String::from("EFFECTOR POLICY: A RULE, NOT A COMMENT")) };
    if wanted.0 == "effector-policy" { return Err(String::from("EFFECTOR POLICY: THE FIRST LINE NAMES THE GRAMMAR'S VERSION; IT IS NOT A RULE")); }
    let current = current.unwrap_or("");
    let mut out = String::new();
    let mut change = None;
    let mut header = false;
    for old in current.lines() {
        let old = old.trim_end_matches('\r');
        header |= old.trim() == HEADER;
        if change.is_none() && identity(old) == Some(wanted) {
            change = Some(if allow { Change::Replaced { old: String::from(old.trim()), new: rule.clone() } } else { Change::Removed(String::from(old.trim())) });
            if allow { out.push_str(&rule); out.push('\n'); }
            continue;
        }
        out.push_str(old);
        out.push('\n');
    }
    if !header && current.trim().is_empty() { out = alloc::format!("{}\n{}", HEADER, out.trim_start()); }
    let change = match change {
        Some(change) => change,
        None if allow => { out.push_str(&rule); out.push('\n'); Change::Added(rule.clone()) }
        None => return Err(alloc::format!("EFFECTOR POLICY: NO RULE {} TO DENY: NOT CHANGED", rule)),
    };
    match check(&out) {
        Ok(rules) => Ok((out, change, rules)),
        Err((at, why)) => Err(alloc::format!("EFFECTOR POLICY: NOT CHANGED: THE NEW POLICY WOULD BE REFUSED AT LINE {} ({}), AND A REFUSED POLICY ALLOWS NOTHING", at, why)),
    }
}

/// The question before an `allow`, which may widen what the server may do here.
pub fn question(rule: &str) -> String {
    alloc::format!("ALLOW `{}` IN THE EFFECTOR POLICY? THE EFFECTOR SERVER MAY THEN DO ON THIS MACHINE WHAT THE RULE NAMES (DOCS/EFFECTOR/POLICY.MD).", rule)
}

/// The line after an edit, and when the gateway reads it: `reloaded` is the rules' count the gateway now has, if the
/// shell could ask it.
pub fn done(change: &Change, rules: u16, reloaded: Option<u16>) -> String {
    let what = match change {
        Change::Added(new) => alloc::format!("+ {}", new),
        Change::Replaced { old, new } => alloc::format!("{} (WAS {})", new, old),
        Change::Removed(old) => alloc::format!("- {}", old),
    };
    let when = match reloaded {
        Some(count) => alloc::format!("THE GATEWAY READ IT: {} RULES", count),
        None => String::from("THE GATEWAY READS IT AT ITS NEXT START (A RESTART, OR A MODE TURNED OFF AND ON); INIT'S OWN CHECKS AT ONCE"),
    };
    alloc::format!("EFFECTOR POLICY: {}; {} RULES, KEPT IN {}. {}", what, rules, FILE, when)
}

/// The words for a parse error's kind.
pub fn why(kind: &str) -> &'static str {
    match kind {
        "TooLarge" => "OVER 4096 BYTES OR 64 RULES",
        "NotText" => "NOT UTF-8 TEXT",
        "Version" => "NO `effector-policy 1` FIRST",
        "LineTooLong" => "A LINE OVER 200 BYTES",
        "UnknownKey" => "A KEY THE GRAMMAR DOES NOT KNOW",
        "Malformed" => "A RULE'S WORDS ARE WRONG",
        "Duplicate" => "A RULE THAT COMES ONCE, TWICE",
        "TooMany" => "TOO MANY OF ONE RULE",
        "Protected" => "A PROTECTED NAME OR TREE",
        _ => "REFUSED",
    }
}

/// What `effector policy` shows below the file: whether it parses, as the gateway would read it.
pub fn state(present: bool, parsed: Result<u16, (u16, &'static str)>) -> String {
    match (present, parsed) {
        (false, _) => alloc::format!("NO {}: THE GATEWAY ALLOWS NOTHING. EFFECTOR POLICY ALLOW <LINE> WRITES ONE", FILE),
        (true, Ok(rules)) => alloc::format!("THE POLICY PARSES: {} RULES", rules),
        (true, Err((line, why))) => alloc::format!("THE POLICY IS REFUSED AT LINE {} ({}): THE GATEWAY ALLOWS NOTHING", line, why),
    }
}
