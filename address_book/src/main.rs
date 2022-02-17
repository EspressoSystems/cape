use address_book::init_web_server;
use tide::log::LevelFilter;

/// Run a web server that provides a key/value store mapping user
/// addresses to public keys.
#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    init_web_server(LevelFilter::Info)
        .await
        .unwrap_or_else(|err| {
            panic!("Web server exited with an error: {}", err);
        });
    Ok(())
}
