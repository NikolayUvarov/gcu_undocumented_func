//! The Effector agent's two services (750-EFF-0006, 0007): the badges of `effector_gw`'s clients and the slots `init`
//! fills for each service (750-KRN-0119), so `init`, the shell (750-APP-0093) and the services agree on one table.

/// The session's client of `effector_gw` (`effector`): the actions of `idl/effector.wit`, never the owner's calls.
pub const BADGE_SESSION: u16 = 1;
/// The owner's client of `effector_gw` (the shell): `status`, `reload-policy` and `audit`, no action.
pub const BADGE_OWNER: u16 = 2;

/// In `effector`'s slots, besides `SLOT_RTC`, `SLOT_TLS`, `SLOT_PARSE` and `SLOT_NETWORK`: its client of `effector_gw`.
pub const SLOT_GATEWAY: usize = 4;
/// In `effector_gw`'s slots, besides `SLOT_SYSINFO`, `SLOT_LIFECYCLE` and `SLOT_LOG`: a reading client of the volumes.
pub const SLOT_READER: usize = 5;

/// `effector`'s private directory (its `SLOT_VFS` client): the token, the agent's ID and the ACK journal.
pub const SESSION_DIR: &str = "system/effector";
/// `effector_gw`'s private directory (its `SLOT_VFS` client): the audit file.
pub const GATEWAY_DIR: &str = "system/effector_gw";
/// The owner's directory, which both read: `effector.conf`, `policy`, and the token the shell leaves for the session.
pub const OWNER_DIR: &str = "data/effector";
/// The owner's policy, which the gateway and `init` both read (750-KRN-0121).
pub const POLICY_FILE: &str = "data/effector/policy";
/// Where the gateway installs signed application packages, `<name>/<version>/` and `<name>/current` (750-UPD-0019);
/// its writable scope of it is SLOT_APPS, and the loader checks a program there before it runs it (750-KRN-0169).
pub const APPS_DIR: &str = "data/apps";

/// The owner's policy (750-EFF-0003): `effector_gw` decides each request with it, and `init` the gateway's lifecycle
/// requests and its delivery scope (750-KRN-0120, 0121), so both read it with one parser.
pub mod policy;
