// Copyright © 2020 Translucence Research, Inc. All rights reserved.

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
