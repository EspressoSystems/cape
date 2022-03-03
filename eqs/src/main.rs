use eqs::configuration::EQSOptions;
use structopt::StructOpt;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt().pretty().init();
    eqs::run_eqs(&EQSOptions::from_args()).await
}
