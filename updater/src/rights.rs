//! Which of `update.wit`'s calls a client may make, by its badge (751-KRN-0185, 751-UPD-0032, 100003-UPD-0052; MC-3.11).

/// A call by what it does: asks the source (`check`); changes the slots (`fetch`, `apply`, `rollback`); hands over a
/// release as a bundle (`install-…`). Every client may read the updater's state (`status`, `metadata`, `kept`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Call { Check, Change, Bundle }

/// What the badge lets a call do: be answered, be `denied`, or be answered while init's `remote-update` counts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule { Answer, Deny, WhileRemoteUpdate }

/// The rule for `call` from a client badged `badge`, given the gateway's (`mind::process`). The shell's client has no
/// badge (0) and may do all but hand over a bundle; any other badge, the reading client's among them, may do none.
pub fn rule(call: Call, badge: u16, gateway: u16) -> Rule {
    match (call, badge) {
        (Call::Check, b) if b == 0 || b == gateway => Rule::Answer,
        (Call::Change | Call::Bundle, b) if b == gateway => Rule::WhileRemoteUpdate,
        (Call::Change, 0) => Rule::Answer,
        _ => Rule::Deny,
    }
}
