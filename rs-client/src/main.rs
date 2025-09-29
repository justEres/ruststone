use rs_protocol::protocol;
use tracing::info;


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
    .without_time()
    .compact()
    .init();

    

    info!("Starting ruststone")
}
