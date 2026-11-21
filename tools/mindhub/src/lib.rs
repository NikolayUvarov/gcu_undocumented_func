//! `mindhub`: MIND Hub's separate server (docs/hub/README.md, 100003-APP-0146), on Linux. Users' requests and their
//! conversation, registration with the admin's answer, trust and anti-flood, the board and the agents' API, over
//! `hub-api 1` with `improve/proto`'s forms.

pub mod api;
pub mod client;
pub mod config;
pub mod conformance;
pub mod event;
pub mod http;
pub mod hub;
pub mod journal;
pub mod pages;
pub mod server;
pub mod util;
