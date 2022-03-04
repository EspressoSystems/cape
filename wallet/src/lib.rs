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

/// An implementation of [seahorse::WalletBackend] for CAPE.
pub mod backend;
/// Configurable API loading.
pub mod disco;
/// Web server endpoint handlers.
pub mod routes;
/// Instantiation of [seahorse::Wallet] for CAPE.
pub mod wallet;
/// Web server.
pub mod web;

/// Testing utility for the CLI.
///
/// DEPRECATED: DO NOT USE THIS IN NEW CODE.
#[cfg(any(test, feature = "testing"))]
pub mod cli_client;
/// Test-only implementation of the [reef] ledger abstraction for CAPE.
#[cfg(any(test, feature = "testing"))]
pub mod mocks;
/// Testing utilities.
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use wallet::*;
