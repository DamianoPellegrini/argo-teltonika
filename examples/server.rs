use std::io;

use argo_teltonika::{Argo, DebugPlugin, Plugin, TeltonikaEvent};

struct TestRedis;

impl Plugin for TestRedis {
    fn can_teltonika_connect(&mut self, imei: &str) -> bool {
        log::info!("{} can connect", imei);
        true
    }

    fn on_teltonika_connected(&mut self, imei: &str) {
        log::info!("{} connected", imei);
    }

    fn on_teltonika_disconnected(&mut self, imei: &str) {
        log::info!("{} disconnected", imei);
    }

    fn on_teltonika_event(&mut self, imei: &str, _event: &TeltonikaEvent) {
        log::info!("{} sent an event", imei);
    }
}

fn main() -> io::Result<()> {
    env_logger::init();
    let mut a = Argo::new("127.0.0.1:56552");

    a.add_plugin(DebugPlugin);
    // a.add_plugin(TestRedis);
    a.run()?;
    Ok(())
}
