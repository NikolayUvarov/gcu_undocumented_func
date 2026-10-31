//! What mindlink and effector485 share as the link's master beyond linkproto: the keys INPUT sends (800-LNK-0032),
//! built by MIND Core's own PS/2 decoder so they are the events its keyboard would give.
#[allow(dead_code, clippy::all)]
#[path = "../../../common/abi.rs"]
mod abi;
#[allow(dead_code, clippy::all)]
#[path = "../../../libmind/src/keys.rs"]
mod ps2;
pub mod keys;
