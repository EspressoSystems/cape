use crate::txn_queue::TxnQueue;

use async_std::sync::{Arc, RwLock};
use async_std::task;
use tide::StatusCode;

#[derive(Clone)]
pub struct WebState {
    txn_queue: Arc<RwLock<TxnQueue>>,
}

async fn submit_endpoint(mut req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    let tx = server::request_body(&mut req).await?;
    let mut queue = req.state().txn_queue.write().await;
    queue.push(tx);
    Ok(tide::Response::new(StatusCode::Ok))
}

/// Initialize the web server.
///
/// `opt_web_path` is the path to the web assets directory. If the path
/// is empty, the default is constructed assuming Cargo is used to
/// build the executable in the customary location.
///
/// `own_id` is the identifier of this instance of the executable. The
/// port the web server listens on is `50077`, unless the
/// PORT environment variable is set.
const DEFAULT_RELAYER_PORT: u16 = 50077u16;

pub(crate) fn init_web_server(
    txn_queue: Arc<RwLock<TxnQueue>>,
) -> Result<task::JoinHandle<Result<(), std::io::Error>>, tide::Error> {
    let mut web_server = tide::with_state(WebState { txn_queue });
    web_server.at("/submit").post(submit_endpoint);
    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_RELAYER_PORT.to_string());
    let addr = format!("0.0.0.0:{}", port);
    let join_handle = async_std::task::spawn(web_server.listen(addr));
    Ok(join_handle)
}

pub mod server {
    use serde::Deserialize;
    use tide::Request;

    /// Deserialize the body of a request.
    ///
    /// The Content-Type header is used to determine the serialization format.
    pub(crate) async fn request_body<T: for<'de> Deserialize<'de>, S>(
        req: &mut Request<S>,
    ) -> Result<T, tide::Error> {
        if let Some(content_type) = req.header("Content-Type") {
            match content_type.as_str() {
                "application/json" => req.body_json().await,
                "application/octet-stream" => {
                    let bytes = req.body_bytes().await?;
                    bincode::deserialize(&bytes).map_err(|e| {
                        tide::Error::from_str(tide::StatusCode::BadRequest, e.to_string())
                    })
                }
                content_type => Err(tide::Error::from_str(
                    tide::StatusCode::BadRequest,
                    format!("unsupported content type {}", content_type),
                )),
            }
        } else {
            Err(tide::Error::from_str(
                tide::StatusCode::BadRequest,
                "unspecified content type",
            ))
        }
    }
}
