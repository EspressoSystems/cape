use serde::{Deserialize, Serialize};
use snafu::Snafu;
use tide::StatusCode;

#[derive(Clone, Debug, Snafu, Serialize, Deserialize)]
pub enum Error {
    #[snafu(display("failed to deserialize request body: {}", msg))]
    Deserialize { msg: String },

    #[snafu(display("submitted transaction does not form a valid block: {}", msg))]
    BadBlock { msg: String },

    #[snafu(display("error during transaction submission: {}", msg))]
    Submission { msg: String },

    #[snafu(display("transaction was not accepted by Ethereum miners"))]
    Rejected,

    #[snafu(display("internal server error: {}", msg))]
    Internal { msg: String },
}

impl net::Error for Error {
    fn catch_all(msg: String) -> Self {
        Self::Internal { msg }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Deserialize { .. } | Self::BadBlock { .. } => StatusCode::BadRequest,
            Self::Submission { .. } | Self::Rejected | Self::Internal { .. } => {
                StatusCode::InternalServerError
            }
        }
    }
}
