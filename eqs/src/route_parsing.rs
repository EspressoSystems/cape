// should be moved to net repo, with Param being a Net error
use crate::errors::*;

use net::TaggedBlob;
use std::path::PathBuf;
use strum_macros::EnumString;
use tagged_base64::TaggedBase64;

#[derive(Clone, Copy, Debug, EnumString)]
pub enum UrlSegmentType {
    Boolean,
    Hexadecimal,
    Integer,
    TaggedBase64,
    Literal,
}

#[allow(dead_code)]
#[derive(Debug, strum_macros::Display)]
pub enum UrlSegmentValue {
    Boolean(bool),
    Hexadecimal(u128),
    Integer(u128),
    Identifier(TaggedBase64),
    Unparsed(String),
    ParseFailed(UrlSegmentType, String),
    Literal(String),
}

use UrlSegmentValue::*;

#[allow(dead_code)]
impl UrlSegmentValue {
    pub fn parse(ptype: UrlSegmentType, value: &str) -> Option<Self> {
        Some(match ptype {
            UrlSegmentType::Boolean => Boolean(value.parse::<bool>().ok()?),
            UrlSegmentType::Hexadecimal => Hexadecimal(u128::from_str_radix(value, 16).ok()?),
            UrlSegmentType::Integer => Integer(value.parse::<u128>().ok()?),
            UrlSegmentType::TaggedBase64 => Identifier(TaggedBase64::parse(value).ok()?),
            UrlSegmentType::Literal => Literal(String::from(value)),
        })
    }

    pub fn as_boolean(&self) -> Result<bool, tide::Error> {
        if let Boolean(b) = self {
            Ok(*b)
        } else {
            Err(server_error(EQSNetError::Param {
                expected: String::from("Boolean"),
                actual: self.to_string(),
            }))
        }
    }

    pub fn as_index(&self) -> Result<usize, tide::Error> {
        if let Integer(ix) = self {
            Ok(*ix as usize)
        } else {
            Err(server_error(EQSNetError::Param {
                expected: String::from("Index"),
                actual: self.to_string(),
            }))
        }
    }

    pub fn as_u64(&self) -> Result<u64, tide::Error> {
        if let Integer(i) = self {
            Ok(*i as u64)
        } else {
            Err(server_error(EQSNetError::Param {
                expected: String::from("Integer"),
                actual: self.to_string(),
            }))
        }
    }

    pub fn as_identifier(&self) -> Result<TaggedBase64, tide::Error> {
        if let Identifier(i) = self {
            Ok(i.clone())
        } else {
            Err(server_error(EQSNetError::Param {
                expected: String::from("TaggedBase64"),
                actual: self.to_string(),
            }))
        }
    }

    pub fn as_path(&self) -> Result<PathBuf, tide::Error> {
        let tb64 = self.as_identifier()?;
        if tb64.tag() == "PATH" {
            Ok(PathBuf::from(std::str::from_utf8(&tb64.value())?))
        } else {
            Err(server_error(EQSNetError::Tag {
                expected: String::from("PATH"),
                actual: tb64.tag(),
            }))
        }
    }

    pub fn as_string(&self) -> Result<String, tide::Error> {
        match self {
            Self::Literal(s) => Ok(String::from(s)),
            Self::Identifier(tb64) => Ok(String::from(std::str::from_utf8(&tb64.value())?)),
            _ => Err(server_error(EQSNetError::Param {
                expected: String::from("String"),
                actual: self.to_string(),
            })),
        }
    }

    pub fn to<T: TaggedBlob>(&self) -> Result<T, tide::Error> {
        T::from_tagged_blob(&self.as_identifier()?).map_err(|err| {
            server_error(EQSNetError::Deserialize {
                msg: err.to_string(),
            })
        })
    }
}

#[derive(Debug)]
pub struct RouteBinding {
    /// Placeholder from the route pattern, e.g. :id
    pub parameter: String,

    /// Type for parsing
    pub ptype: UrlSegmentType,

    /// Value
    pub value: UrlSegmentValue,
}
