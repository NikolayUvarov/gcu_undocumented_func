#![no_std]
//! The Effector gateway's policy, audit record and decisions (750-EFF-0003, 0007); its service binds them to `init`'s
//! grants once 750-KRN-0119 gives it a boot image.
extern crate alloc;
pub use mind::sha256;
pub mod audit;
pub mod gateway;
pub use mind::effector::policy;
