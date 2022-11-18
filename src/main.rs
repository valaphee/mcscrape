use std::error::Error;

use influxdb::{Client, InfluxDbWriteable, Timestamp};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(InfluxDbWriteable)]
struct PlayerCount {
    time: Timestamp,
    #[influxdb(tag)]
    server: String,
    player_count: u32
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("heee");

    let client = Client::new("http://localhost:8086", "mcscrape");

    client.query(
        PlayerCount {
            time: Timestamp::Hours(1).into(),
            server: "test".to_string(),
            player_count: 1
        }.into_query("player_count")
    ).await?;

    Ok(())
}
