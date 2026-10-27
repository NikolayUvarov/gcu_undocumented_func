//! A program named instead of its PID (`fg wm`, the maintainer's request of 2026-10-22): the programs whose name is the
//! text, in any case, or else those whose name holds it. One is taken; several are listed with their PIDs to choose from.

/// The programs among `tasks` (PID, name) that `text` names: those called so, or else those whose name holds it.
pub fn matching<'a, I>(text: &'a str, tasks: I) -> impl Iterator<Item = (u64, &'a str)>
where I: Iterator<Item = (u64, &'a str)> + Clone {
    let exact = tasks.clone().any(|(_, name)| name.eq_ignore_ascii_case(text));
    tasks.filter(move |(_, name)| if exact { name.eq_ignore_ascii_case(text) } else { holds(name, text) })
}

/// What `matching` gave: one PID, none, or several (listed by the caller).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked { One(u64), None, Several }

pub fn picked<'a>(found: impl Iterator<Item = (u64, &'a str)>) -> Picked {
    let mut found = found;
    match (found.next(), found.next()) { (None, _) => Picked::None, (Some((pid, _)), None) => Picked::One(pid), _ => Picked::Several }
}

// Whether `name` holds `text`, in any case.
fn holds(name: &str, text: &str) -> bool {
    !text.is_empty() && name.as_bytes().windows(text.len()).any(|window| window.eq_ignore_ascii_case(text.as_bytes()))
}
