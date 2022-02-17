use address_book::init_web_server;
use async_std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Address Book",
    about = "Server that provides a key/value store mapping user addresses to public keys"
)]
struct ServerOpt {
    /// Whether to load from persisted state. Defaults to true.
    ///
    #[structopt(
        long = "load_from_store",
        short = "l",
        parse(try_from_str),
        default_value = "true"
    )]
    load_from_store: bool,

    /// Path to persistence files.
    ///
    /// Persistence files will be nested under the specified directory
    #[structopt(
        long = "store_path",
        short = "s",
        default_value = ""      // See fn default_store_path().
    )]
    store_path: String,

    /// Base URL. Defaults to http://0.0.0.0:50078.
    #[structopt(long = "url", default_value = "http://0.0.0.0:50078")]
    base_url: String,
}

/// Returns the project directory.
fn project_path() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    println!("path {}", path.display());
    path
}

/// Returns the default directory to store persistence files.
fn default_store_path() -> PathBuf {
    const STORE_DIR: &str = "src/store/address_book";
    let dir = project_path();
    [&dir, Path::new(STORE_DIR)].iter().collect()
}

/// Gets the directory to public key files.
// TODO !corbett why return a string when you could return a Path
fn get_store_dir() -> String {
    let store_path = ServerOpt::from_args().store_path;
    if store_path.is_empty() {
        default_store_path()
            .into_os_string()
            .into_string()
            .expect("Error while converting store path to a string")
    } else {
        store_path
    }
}

/// Run a web server that provides a key/value store mapping user
/// addresses to public keys.
#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    let store = if ServerOpt::from_args().load_from_store {
        Some(ServerOpt::from_args().store_path)
    } else {
        None
    };
    init_web_server(&ServerOpt::from_args().base_url, store)
        .await
        .unwrap_or_else(|err| {
            panic!("Web server exited with an error: {}", err);
        });
    Ok(())
}
