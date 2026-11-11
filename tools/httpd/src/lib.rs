//! `httpd`: the project's own web server for releases (110, stage 1), on Linux; its parts, tested on the host.

pub mod date;
pub mod head;
#[cfg(feature = "test-hooks")]
pub mod hooks;
pub mod layout;
pub mod log;
pub mod range;
pub mod server;
pub mod tls;
