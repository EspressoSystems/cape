#![warn(unused_imports)]
// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # CAPE Wallet
//!
//! This crate contains an instantiation of the generic [seahorse] wallet framework for CAPE. It
//! also extends the generic wallet interface with CAPE-specific functionality related to wrapping
//! and unwrapping ERC-20 tokens.
//!
//! The instantiation of [seahorse] for CAPE is contained in the modules [wallet] and [backend]. As
//! entrypoints to the wallet, we provide a CLI and a web server as separate executables.

pub mod backend;
pub mod disco;
pub mod ui;
pub mod wallet;

#[cfg(any(test, feature = "testing"))]
pub mod cli_client;
#[cfg(any(test, feature = "testing"))]
pub mod mocks;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use wallet::*;
