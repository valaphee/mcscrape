use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use bincode::config::{BigEndian, Configuration, Fixint, SkipFixedArrayLength};
use influxdb::{Client, InfluxDbWriteable, Timestamp};
use serde::{Deserialize, Serialize};

mod raknet;

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
                influx_db_username: "mcscrape".to_string(),
                influx_db_password: "mcscrape".to_string(),
                targets: vec![]
            }
        };
        serde_json::to_writer_pretty(File::create(config_path)?, &config)?;
        config
    };

    let influxdb_client = Client::new(&config.influx_db_url, &config.influx_db_database).with_auth(&config.influx_db_username, &config.influx_db_password);

    let raknet_bincode_config: Configuration<BigEndian, Fixint, SkipFixedArrayLength> = Configuration::default();
    let mut raknet_client = raknet::RakNetClient::new().await;
    let raknet_guid = rand::random();
    let mut raknet_ping_id: u64 = 0;
    let mut raknet_pings: HashMap<u64, (u64, &ConfigTarget)> = HashMap::new();

    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;

        for config_target in &config.targets {
            match config_target.kind.as_str() {
                "raknet" => {
                    let elapsed_time = raknet_client.start_time.elapsed()?.as_millis() as u64;
                    raknet_client.socket.send_to(&bincode::encode_to_vec(raknet::Packet::UnconnectedPing {
                        elapsed_time: raknet_ping_id,
                        client_guid: raknet_guid
                    }, raknet_bincode_config).unwrap(), &config_target.target).await.unwrap();
                    raknet_pings.insert(raknet_ping_id, (elapsed_time, &config_target));
                    raknet_ping_id += 1;
                }
                _ => ()
            }
        }

        while let Ok((duration, packet)) = raknet_client.rx.try_recv() {
            match packet {
                raknet::Packet::UnconnectedPong { elapsed_time: raknet_ping_id, server_guid: _, user_data } => {
                    let status: Vec<&str> = user_data.split(';').collect();
                    if status.len() >= 6 {
                        match raknet_pings.remove(&raknet_ping_id) {
                            Some((elapsed_time, config_target)) => {
                                influxdb_client.query(Ping {
                                    time: Timestamp::Seconds((raknet_client.start_time.duration_since(UNIX_EPOCH)?.as_secs() + duration.as_secs()) as u128),
                                    kind: config_target.kind.clone(),
                                    name: config_target.name.clone(),
                                    player_count: status[4].parse().unwrap(),
                                    player_limit: status[5].parse().unwrap(),
                                    rtt: ((duration.as_millis() as u64) - elapsed_time) as u16
                                }.into_query("ping")).await?;
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

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    influx_db_url: String,
    influx_db_database: String,
    influx_db_username: String,
    influx_db_password: String,
    targets: Vec<ConfigTarget>
}

#[derive(Serialize, Deserialize, Debug)]
struct ConfigTarget {
    kind: String,
    name: String,
    target: String
}

#[derive(InfluxDbWriteable, Debug)]
struct Ping {
    time: Timestamp,
    #[influxdb(tag)]
    kind: String,
    #[influxdb(tag)]
    name: String,
    player_count: u32,
    player_limit: u32,
    rtt: u16
}
