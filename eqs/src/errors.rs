// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use serde::{Deserialize, Serialize};
use snafu::Snafu;
use tide::StatusCode;

#[derive(Debug, Snafu, Serialize, Deserialize)]
#[snafu(module(error))]
pub enum EQSNetError {
    #[snafu(display("invalid parameter: expected {}, got {}", expected, actual))]
    Param { expected: String, actual: String },

    #[snafu(display("invalid TaggedBase64 tag: expected {}, got {}", expected, actual))]
    Tag { expected: String, actual: String },

    #[snafu(display("failed to deserialize request parameter: {}", msg))]
    Deserialize { msg: String },

    #[snafu(display("internal server error: {}", msg))]
    Internal { msg: String },
}

impl net::Error for EQSNetError {
    fn catch_all(msg: String) -> Self {
        Self::Internal { msg }
    }
    fn status(&self) -> StatusCode {
        match self {
            Self::Param { .. } | Self::Tag { .. } | Self::Deserialize { .. } => {
                StatusCode::BadRequest
            }
            Self::Internal { .. } => StatusCode::InternalServerError,
        }
    }
}

pub fn server_error<E: Into<EQSNetError>>(err: E) -> tide::Error {
    net::server_error(err)
}
