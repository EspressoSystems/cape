#![warn(unused_imports)]
//! # CAPE Wallet
//!
//! This crate contains an instantiation of the generic [seahorse] wallet framework for CAPE. It
//! also extends the generic wallet interface with CAPE-specific functionality related to wrapping
//! and unwrapping ERC-20 tokens.
//!
//! The instantiation of [seahorse] for CAPE is contained the modules [wallet] and [backend]. As
//! entrypoints to the wallet, we provide a CLI and a web server as separate executables, but much
//! of the functionality of the web server is included in this crate as a library, in the modules
//! [disco], [routes], and [web].

pub mod backend;
pub mod disco;
pub mod routes;
pub mod wallet;
pub mod web;

#[cfg(any(test, feature = "testing"))]
pub mod cli_client;
#[cfg(any(test, feature = "testing"))]
pub mod mocks;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use wallet::*;
