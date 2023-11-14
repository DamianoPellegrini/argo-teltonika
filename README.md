# Argo

A pluggable, lightweight, and fast TCP server framework for [Teltonika GPS](https://teltonika-gps.com) devices.

## Features

- Logging via the [log](https://docs.rs/log) crate.

Exposed by the [Plugin](src/plugins.rs#Plugin) trait:

- Filtering connection based on IMEI.
- Connect, Disconnect & [TeltonikaEvent](src/lib.rs#TeltonikaEvent) handlers.

Cargo related:

- `async`: Enables async support leveraging the [tokio](https://docs.rs/tokio) crate.

## Examples

```rust no_run
use std::io;

use argo_teltonika::{plugins::{Plugin, DebugPlugin}, TeltonikaEvent, Argo};

struct CounterPlugin {
    count: usize,
}

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

fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(|panic_info| {
        log::error!("{}", panic_info);
    }));

    env_logger::init();
    let mut app = Argo::new("127.0.0.1:56552");

    app.add_plugin(DebugPlugin);
    app.add_plugin(CounterPlugin { count: 0 });
    app.run()?;
    Ok(())
}
```

*Further examples can be found in the [examples folder](examples/).*
