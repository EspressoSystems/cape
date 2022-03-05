// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use serde_derive::{Deserialize, Serialize};
use std::str::FromStr;

/// Network port number
#[derive(Debug, Deserialize, Serialize)]
pub struct IpPort(pub u16);

impl FromStr for IpPort {
    type Err = std::num::ParseIntError;

    fn from_str(port_str: &str) -> Result<Self, Self::Err> {
        Ok(IpPort(u16::from_str(port_str)?))
    }
}
