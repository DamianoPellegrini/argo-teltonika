use std::error::Error;

use argo_teltonika::{Argo, Plugin, TeltonikaEvent};
use redis::Commands;

struct RedisPlugin {
    _client: redis::Client,
    conn: redis::Connection,
}

impl RedisPlugin {
    fn new(info: impl redis::IntoConnectionInfo) -> Result<Self, Box<dyn Error>> {
        let client = redis::Client::open(info)?;
        let conn = client.get_connection()?;

        Ok(Self {
            _client: client,
            conn,
        })
    }
}

impl Plugin for RedisPlugin {
    fn can_teltonika_connect(&mut self, imei: &str) -> bool {
        log::info!("{} can connect", imei);

        self.conn
            .lpos("allowed", imei, redis::LposOptions::default())
            .unwrap_or(-1) >= 0
    }

    fn on_teltonika_connected(&mut self, imei: &str) {
        let _: usize = self.conn.lpush("connected", imei).unwrap_or(0);
    }

    fn on_teltonika_disconnected(&mut self, imei: &str) {
        let _: usize = self.conn.lrem("connected", 0, imei).unwrap_or(0);
    }

    fn on_teltonika_event(&mut self, imei: &str, event: &TeltonikaEvent) {
        // Serialize and save
        let _: usize = self
            .conn
            .lpush(format!("events:{imei}"), format!("{}: {:?}", imei, event))
            .unwrap_or(0);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    std::panic::set_hook(Box::new(|panic_info| {
        log::error!("{}", panic_info);
    }));

    env_logger::init();
    let mut a = Argo::new("127.0.0.1:56552");

    a.add_plugin(RedisPlugin::new("redis://127.0.0.1")?);

    a.run()?;

    Ok(())
}
