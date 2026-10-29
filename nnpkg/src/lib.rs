//! nnd's checks of `nrl.model/0` packages and its runtime views (262-APP-0112, docs/nn/runtime.md 1.0): a strict JSON
//! reader, the typed description, the writer profile `nrl.serde-json-pretty-object/0`, `nrl-lab`'s checks and the
//! model ID, ported to `no_std` so the model service runs them on MIND Core.
#![no_std]
extern crate alloc;

pub mod check;
pub mod encode;
pub mod json;
pub mod layout;
pub mod model;
pub mod package;
pub mod program;
pub mod service;
#[path = "../../libmind/src/sha256.rs"]
pub mod sha256;

/// Why a package or a request is refused, with the reason nnd logs; each maps to an error of `nn.wit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal { NotFound(&'static str), Digest(&'static str), Malformed(&'static str), Unsupported(&'static str), TooLarge(&'static str) }

impl Refusal {
    pub fn reason(self) -> &'static str {
        match self { Self::NotFound(r) | Self::Digest(r) | Self::Malformed(r) | Self::Unsupported(r) | Self::TooLarge(r) => r }
    }
}
