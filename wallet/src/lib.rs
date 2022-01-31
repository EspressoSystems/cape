pub mod backend;
pub mod cli_client;
pub mod disco;
pub mod mocks;
pub mod routes;
pub mod wallet;
pub mod web;

pub use wallet::*;

pub mod testing {
    use async_std::sync::{Arc, Mutex};
    use lazy_static::lazy_static;

    lazy_static! {
        static ref PORT: Arc<Mutex<u64>> = {
            let port_offset = std::env::var("PORT").unwrap_or_else(|_| String::from("60000"));
            Arc::new(Mutex::new(port_offset.parse().unwrap()))
        };
    }

    pub async fn port() -> u64 {
        let mut counter = PORT.lock().await;
        let port = *counter;
        *counter += 1;
        port
    }
}
