# Argo

A pluggable, lightweight, and fast TCP server framework for [Teltonika GPS](https://teltonika-gps.com) devices.

> [!CAUTION]
> Test are not yet implemented, so this crate will not be published to crates.io until they are.
>
> Can still be used as a git dependency. Beware of breaking changes, or bugs in general.
>
> _Features will be implemented during my free time when i feel like it. Don't expect this to be an actively maintained project, I suggest forking it if needed for any production use._

## Features

- Logging via the [log](https://docs.rs/log) crate and [DebugPlugin](src/plugin.rs#DebugPlugin).

- Exposed by the [Plugin](src/plugin.rs#Plugin) trait:
  - Filtering connection based on IMEI.
  - Connect, Disconnect & [TeltonikaEvent](src/plugin.rs#TeltonikaEvent) handlers.
  - Startup & Shutdown using standard rust `::new()` custom fn or `Default` trait and `Drop` trait implementations.

### Planned features

- [ ] More robust API:
  - [ ] custom error types
  - [ ] rethink plugin ownership
  - [ ] review multi-threading model and safety.
- [ ] Support for async:
  - [ ] AsyncPlugins
  - [ ] AsyncServer
- [ ] Tests.
- [ ] Support for the [tracing](https://docs.rs/tracing) crate.
- [ ] Support for UDP connections.
- [ ] Support for custom packet parsing.
- [ ] More complex examples
  - [x] Redis server
  - [ ] HTTP server
  - [ ] Websocket server

## Examples

```rust no_run
use std::io;

use argo_teltonika::{Argo, DebugPlugin, Plugin, TeltonikaEvent};

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
        log::info!("{} sent an event, current connections: {}", imei, self.count);
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

*Further examples, such as a redis connected server, can be found in the [examples folder](examples/).*
