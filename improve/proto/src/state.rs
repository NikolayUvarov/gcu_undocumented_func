//! A request's states and who moves it between them (docs/hub/README.md, 6.1). A move not in the table is refused, so a
//! state never goes backwards unless the table names who may do that, and the hub records each move with its reason.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    Held,
    New,
    NeedsInfo,
    Rejected,
    Duplicate,
    Accepted,
    InProgress,
    Landed,
    Delivered,
    AwaitingCheck,
    Verified,
    Reopened,
    ClosedUnchecked,
}

pub const STATES: [State; 13] = [
    State::Held, State::New, State::NeedsInfo, State::Rejected, State::Duplicate, State::Accepted, State::InProgress,
    State::Landed, State::Delivered, State::AwaitingCheck, State::Verified, State::Reopened, State::ClosedUnchecked,
];

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Held => "held",
            State::New => "new",
            State::NeedsInfo => "needs-info",
            State::Rejected => "rejected",
            State::Duplicate => "duplicate",
            State::Accepted => "accepted",
            State::InProgress => "in-progress",
            State::Landed => "landed",
            State::Delivered => "delivered",
            State::AwaitingCheck => "awaiting-check",
            State::Verified => "verified",
            State::Reopened => "reopened",
            State::ClosedUnchecked => "closed-unchecked",
        }
    }

    pub fn from_name(name: &str) -> Option<State> { STATES.into_iter().find(|s| s.name() == name) }

    /// Whether the board shows a request in this state: only what became a task (docs/hub/README.md, 6.4).
    pub fn on_board(self) -> bool {
        matches!(self, State::Accepted | State::InProgress | State::Landed | State::Delivered | State::AwaitingCheck | State::Verified | State::Reopened | State::ClosedUnchecked)
    }

    /// No one moves a request out of it but the admin (or, for a closed one, its user's verdict).
    pub fn closed(self) -> bool { matches!(self, State::Rejected | State::Duplicate | State::Verified | State::ClosedUnchecked) }
}

/// Who asks for a move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The hub itself: a filing, a device's approval or rejection, a release, a device's version, a timeout.
    Hub,
    /// The request's own user.
    User,
    /// The intake agent.
    Intake,
    /// The owning track's agent.
    Track,
    /// The integrator's agent, which lands it.
    Integrator,
    /// The admin.
    Admin,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self { Role::Hub => "hub", Role::User => "user", Role::Intake => "intake", Role::Track => "track", Role::Integrator => "integrator", Role::Admin => "admin" }
    }
}

/// Whether `role` may move a request from `from` to `to`. `NeedsInfo` goes back, on the user's answer, to the state the
/// question was asked in, which the hub keeps: the table allows each of those.
pub fn allowed(from: State, to: State, role: Role) -> bool {
    use State::*;
    match role {
        Role::Hub => matches!((from, to),
            (Held, New) | (Held, Rejected) | (Landed, Delivered) | (Delivered, AwaitingCheck) | (AwaitingCheck, ClosedUnchecked)),
        Role::User => matches!((from, to),
            (NeedsInfo, New) | (NeedsInfo, Accepted) | (NeedsInfo, InProgress) | (NeedsInfo, Reopened)
            | (AwaitingCheck, Verified) | (AwaitingCheck, Reopened) | (ClosedUnchecked, Reopened)),
        Role::Intake => matches!((from, to), (New, NeedsInfo) | (New, Rejected) | (New, Duplicate) | (New, Accepted)),
        Role::Track => matches!((from, to),
            (Accepted, InProgress) | (Reopened, InProgress)
            | (Accepted, NeedsInfo) | (InProgress, NeedsInfo) | (Reopened, NeedsInfo)),
        Role::Integrator => matches!((from, to), (InProgress, Landed)),
        // The admin does what intake does, decides a held request, rejects anything open (moderation, spam), and takes a
        // closed request back to intake, with its reason.
        Role::Admin => allowed(from, to, Role::Intake)
            || matches!((from, to), (Held, New) | (Held, Rejected))
            || (to == Rejected && !from.closed())
            || (to == New && matches!(from, Rejected | Duplicate)),
    }
}

/// The state a request starts in: held for a device not approved yet, new otherwise.
pub fn first(approved: bool) -> State { if approved { State::New } else { State::Held } }
