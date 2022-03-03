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

/// Instantiation of [seahorse::Wallet] for CAPE.
pub mod wallet;

/// An implementation of [seahorse::WalletBackend] for CAPE.
pub mod backend;

/// Web server.
pub mod web;

/// Web server endpoint handlers.
pub mod routes;

/// Configurable API loading.
pub mod disco;

/// Testing utilities.
#[cfg(any(test, feature = "testing"))]
pub mod testing;

/// Test-only implementation of the [reef] ledger abstraction for CAPE.
#[cfg(any(test, feature = "testing"))]
pub mod mocks;

/// Testing utility for the CLI.
///
/// DEPRECATED: DO NOT USE THIS IN NEW CODE.
#[cfg(any(test, feature = "testing"))]
pub mod cli_client;

pub use wallet::*;
