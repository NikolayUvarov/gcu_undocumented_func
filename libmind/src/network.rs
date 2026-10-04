//! Badges of network stack clients (idl/socket.wit 2.0, issue 102): who may do what is part of the capability.

/// The operator (the shell): every destination, no term or volume.
pub const BADGE_OPERATOR: u16 = 0xFFFE;
/// The policy broker's control client: registers and drops grants, opens no flows.
pub const BADGE_POLICY: u16 = 0xFFFF;
/// Flow grants of the broker use badges from 1 up to this one.
pub const BADGE_GRANT_LAST: u16 = 0xFFFD;
