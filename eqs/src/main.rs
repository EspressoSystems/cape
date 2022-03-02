mod api_server;
mod configuration;
mod disco; // really needs to go into a shared crate
mod eqs;
mod errors;
mod eth_polling;
mod query_result_state;
mod route_parsing;
mod routes;
mod state_persistence;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt().pretty().init();
    eqs::run_eqs().await
}
