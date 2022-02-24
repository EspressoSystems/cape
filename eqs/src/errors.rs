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
