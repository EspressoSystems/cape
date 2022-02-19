use address_book::{default_store_path, init_web_server, ServerOpt};
use async_std::path::PathBuf;
use signal_hook::{consts::SIGTERM, iterator::Signals};
use std::process;
use std::thread;
use structopt::StructOpt;
use tide::log::LevelFilter;

/// Run a web server that provides a key/value store mapping user
/// addresses to public keys.
#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    let mut signals = Signals::new(&[SIGTERM])?;
    thread::spawn(move || {
        for sig in signals.forever() {
            println!("Received signal {:?}", sig);
            process::exit(1);
        }
    });

    let store_path = if ServerOpt::from_args().store_path.is_empty() {
        default_store_path()
    } else {
        PathBuf::from(&ServerOpt::from_args().store_path[..])
    };
    let opt_store_path = if ServerOpt::from_args().load_from_store {
        Some(&store_path)
    } else {
        None
    };
    let base_url = &ServerOpt::from_args().base_url[..];
    let handle = init_web_server(LevelFilter::Error, &base_url, opt_store_path)
        .await
        .unwrap_or_else(|err| panic!("Web server exited with an error: {}", err));
    handle
        .await
        .unwrap_or_else(|err| panic!("Web server exited with an error: {}", err));
    Ok(())
}
