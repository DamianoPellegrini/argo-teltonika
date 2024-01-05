use std::io;

use argo_teltonika::{Argo, Plugin, TeltonikaEvent};

struct HashMapPlugin {
    allowlist: Vec<String>,
    map: std::collections::HashMap<String, Option<TeltonikaEvent>>,
}

impl Plugin for HashMapPlugin {

    fn can_teltonika_connect(&mut self, imei: &str) -> bool {
        let can_connect = self.allowlist.iter().any(|a| a == imei);

        if can_connect {
            log::info!("{} can connect", imei);
        } else {
            log::info!("{} can't connect", imei);
        }

        can_connect
    }

    fn on_teltonika_connected(&mut self, imei: &str) {
        self.map.insert(imei.to_string(), None);
        log::info!("{} connected", imei);
    }

    fn on_teltonika_disconnected(&mut self, imei: &str) {
        self.map.remove(imei);
        log::info!("{} disconnected", imei);
    }

    fn on_teltonika_event(&mut self, imei: &str, event: &TeltonikaEvent) {
        if let Some(value) = self.map.get_mut(imei) {
            *value = Some(event.clone());
        };
        log::info!("{} sent an event at {}", imei, event.records[0].timestamp);
    }
}

fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(|panic_info| {
        log::error!("{}", panic_info);
    }));

    env_logger::init();
    let mut a = Argo::new("127.0.0.1:56552");

    a.add_plugin(HashMapPlugin {
        allowlist: vec!["IMEI0123456789A".to_string()],
        // allowlist: vec![],
        map: std::collections::HashMap::new(),
    });

    a.run()?;

    Ok(())
}
