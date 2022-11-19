use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bincode::config::{BigEndian, Configuration, Fixint, SkipFixedArrayLength};
use influxdb::{Client, InfluxDbWriteable, Timestamp};
use mimalloc::MiMalloc;
use serde::{Deserialize, Serialize};

mod raknet;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    influx_db_url: String,
    influx_db_database: String,
    targets: Vec<ConfigTarget>
}

#[derive(Serialize, Deserialize, Debug)]
struct ConfigTarget {
    kind: String,
    target: String
}

#[derive(InfluxDbWriteable, Debug)]
struct Ping {
    time: Timestamp,
    #[influxdb(tag)]
    kind: String,
    #[influxdb(tag)]
    target: String,
    player_count: u32,
    player_limit: u32
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = {
        let config_path = Path::new("config.json");
        let config = if config_path.exists() {
            serde_json::from_reader(File::open(config_path)?)?
        } else {
            Config {
                influx_db_url: "http://localhost:8086".to_string(),
                influx_db_database: "mcscrape".to_string(),
                targets: vec![]
            }
        };
        serde_json::to_writer_pretty(File::create(config_path)?, &config)?;
        config
    };

    let influxdb_client = Client::new(&config.influx_db_url, &config.influx_db_database);

    let raknet_bincode_config: Configuration<BigEndian, Fixint, SkipFixedArrayLength> = Configuration::default();
    let mut raknet_client = raknet::RakNetClient::new().await;
    let raknet_start_time = SystemTime::now();
    let mut raknet_pending: HashMap<u64, &ConfigTarget> = HashMap::new();

    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u128;

        for config_target in &config.targets {
            match config_target.kind.as_str() {
                "raknet" => {
                    let elapsed_time = raknet_start_time.elapsed()?.as_millis() as u64;
                    raknet_client.socket.send_to(&bincode::encode_to_vec(raknet::Packet::UnconnectedPing {
                        elapsed_time,
                        client_guid: 0
                    }, raknet_bincode_config).unwrap(), &config_target.target).await.unwrap();
                    raknet_pending.insert(elapsed_time, config_target);
                }
                _ => ()
            }
        }

        while let Ok(packet) = raknet_client.rx.try_recv() {
            match packet {
                raknet::Packet::UnconnectedPong { elapsed_time, server_guid, user_data } => {
                    let status: Vec<&str> = user_data.split(';').collect();
                    if status.len() >= 6 {
                        match raknet_pending.remove(&elapsed_time) {
                            Some(config_target) => {
                                influxdb_client.query(Ping {
                                    time: Timestamp::Seconds(current_time),
                                    kind: config_target.kind.clone(),
                                    target: config_target.target.clone(),
                                    player_count: status[4].parse().unwrap(),
                                    player_limit: status[5].parse().unwrap()
                                }.into_query("player_count")).await?;
                            }
                            _ => ()
                        }
                    }
                }
                _ => ()
            }
        }
    }
}
