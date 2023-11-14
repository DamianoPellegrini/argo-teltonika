use std::io;

use argo_teltonika::{Argo, DebugPlugin, Plugin, TeltonikaEvent};

struct CounterPlugin {
    count: usize,
}

#[cfg(not(feature = "async"))]
impl Plugin for CounterPlugin {
    fn can_teltonika_connect(&mut self, imei: &str) -> bool {
        log::info!("{} can connect", imei);
        true
    }

    fn on_teltonika_connected(&mut self, imei: &str) {
        self.count += 1;
        log::info!("{} connected {}", imei, self.count);
    }

    fn on_teltonika_disconnected(&mut self, imei: &str) {
        log::info!("{} disconnected {}", imei, self.count);
        self.count -= 1;
    }

    fn on_teltonika_event(&mut self, imei: &str, _event: &TeltonikaEvent) {
        log::info!("{} sent an event {}", imei, self.count);
    }
}

#[cfg(feature = "async")]
#[cfg_attr(feature = "async", async_trait::async_trait)]
impl Plugin for CounterPlugin {
    fn can_teltonika_connect(&mut self, imei: &str) -> bool {
        log::info!("{} can connect", imei);
        true
    }

    async fn on_teltonika_connected(&mut self, imei: &str) {
        self.count += 1;
        log::info!("{} connected {}", imei, self.count);
    }

    async fn on_teltonika_disconnected(&mut self, imei: &str) {
        log::info!("{} disconnected {}", imei, self.count);
        self.count -= 1;
    }

    async fn on_teltonika_event(&mut self, imei: &str, _event: &TeltonikaEvent) {
        log::info!("{} sent an event {}", imei, self.count);
    }
}


#[cfg(not(feature = "async"))]
fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(|panic_info| {
        log::error!("{}", panic_info);
    }));

    env_logger::init();
    let mut a = Argo::new("127.0.0.1:56552");

    a.add_plugin(DebugPlugin);
    a.add_plugin(CounterPlugin { count: 0 });
    a.run()?;
    Ok(())
}


#[cfg(feature = "async")]
#[cfg_attr(feature = "async", tokio::main)]
async fn main() -> io::Result<()>{
    std::panic::set_hook(Box::new(|panic_info| {
        log::error!("{}", panic_info);
    }));

    env_logger::init();
    let mut a = Argo::new("127.0.0.1:56552");

    a.add_plugin(DebugPlugin);
    a.add_plugin(CounterPlugin { count: 0 });
    a.run().await?;
    Ok::<(), std::io::Error>(())
}
