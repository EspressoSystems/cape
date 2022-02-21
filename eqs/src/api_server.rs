use crate::query_result_state::QueryResultState;

use async_std::{
    sync::{Arc, RwLock},
    task,
};
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use tide::StatusCode;

#[derive(Clone, Debug, Snafu, Serialize, Deserialize)]
pub enum Error {
    #[snafu(display("failed to deserialize request body: {}", msg))]
    Deserialize { msg: String },

    #[snafu(display("internal server error: {}", msg))]
    Internal { msg: String },
}

impl net::Error for Error {
    fn catch_all(msg: String) -> Self {
        Self::Internal { msg }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Deserialize { .. } => StatusCode::BadRequest,
            Self::Internal { .. } => StatusCode::InternalServerError,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct WebState {
    query_result_state: Arc<RwLock<QueryResultState>>,
}
/// Initialize the web server.
///
/// `opt_web_path` is the path to the web assets directory. If the path
/// is empty, the default is constructed assuming Cargo is used to
/// build the executable in the customary location.
///
/// `own_id` is the identifier of this instance of the executable. The
/// port the web server listens on is `50087`, unless the
/// PORT environment variable is set.
const DEFAULT_EQS_PORT: u16 = 50087u16;

pub(crate) fn init_web_server(
    query_result_state: Arc<RwLock<QueryResultState>>,
) -> Result<task::JoinHandle<Result<(), std::io::Error>>, tide::Error> {
    let web_server = tide::with_state(WebState { query_result_state });
    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_EQS_PORT.to_string());
    let addr = format!("0.0.0.0:{}", port);
    let join_handle = async_std::task::spawn(web_server.listen(addr));
    Ok(join_handle)
}
