use std::{
    io,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
};

use nom_teltonika::AVLFrame;

use crate::{Plugin, TeltonikaEvent};

pub type PluginHandle = Mutex<Box<dyn Plugin + Send + Sync>>;
pub type PluginHandles = Arc<RwLock<Vec<PluginHandle>>>;

pub struct Argo {
    /// The socket address to listen on
    socket_addr: SocketAddr,

    /// Listeners for events
    plugins: PluginHandles,
}

impl Argo {
    pub fn new(addr: impl std::net::ToSocketAddrs) -> Self {
        Self {
            socket_addr: addr
                .to_socket_addrs()
                .expect("Couldn't parse addr")
                .next()
                .expect("No addr found"),
            plugins: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn add_plugin(&mut self, plugin: impl Plugin + Sync + Send + 'static) {
        self.plugins
            .write()
            .unwrap()
            .push(Mutex::new(Box::new(plugin)));
    }

    pub fn run(&self) -> io::Result<()> {
        let listener = std::net::TcpListener::bind(self.socket_addr)?;

        log::info!("Listening on {}", self.socket_addr);
        
        log::info!("Starting plugins...");
        for plugin in self.plugins.read().unwrap().iter() {
            plugin.lock().expect("Cannot acquire lock").startup();
        }

        let mut handles = vec![];

        // TODO: Add graceful shutdown with ctrl+c and POSIX signals

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let peer_addr = stream.peer_addr()?;
                    log::debug!(
                        target: &format!("{} {peer_addr}", module_path!()),
                        "Connection received"
                    );

                    // Clone a ref and lock as readable
                    let plugins: PluginHandles = self.plugins.clone();

                    let handle =
                        std::thread::spawn(move || handle_connection(stream, plugins, peer_addr));

                    handles.push(handle);
                }
                Err(e) => {
                    log::error!("Network IO error: {}", e);
                    continue;
                }
            }

            handles.retain(|handle| !handle.is_finished());
            handles.shrink_to_fit();
        }

        for handle in handles.into_iter() {
            match handle.join() {
                Ok(r) => {
                    if let Err(e) = r {
                        log::error!("Thread error: {}", e);
                    }
                }
                Err(e) => {
                    log::error!("Thread panicked: {:?}", e);
                }
            }
        }

        log::info!("Shutting down plugins...");
        for plugin in self.plugins.read().unwrap().iter() {
            plugin.lock().expect("Cannot acquire lock").shutdown();
        }

        Ok(())
    }
}

fn handle_connection(
    stream: std::net::TcpStream,
    plugins: PluginHandles,
    peer_addr: std::net::SocketAddr,
) -> io::Result<()> {
    // Clone every plugin ref to be locked
    // Each plugin can be locked by the individual threads
    let plugins = plugins
        .read()
        .expect("Lock is poisoned");
    let mut stream = nom_teltonika::TeltonikaStream::new(stream);
    log::debug!(
        target: &format!("{} {peer_addr}", module_path!()),
        "Connection received"
    );

    let imei = match stream.read_imei() {
        Ok(imei) => imei,
        Err(e) => match e.kind() {
            io::ErrorKind::InvalidData => {
                log::error!(
                    target: &format!("{} {peer_addr}", module_path!()),
                    "Error parsing IMEI, closing connection"
                );

                stream.write_imei_denial()?;
                stream.inner().shutdown(std::net::Shutdown::Both)?;

                return Ok(());
            }
            io::ErrorKind::ConnectionReset => {
                log::debug!(
                    target: &format!("{} {peer_addr}", module_path!()),
                    "Connection reset by peer"
                );

                return Ok(());
            }
            _ => {
                return Err(e);
            }
        },
    };

    let allowed_plugins: Vec<_> = plugins
        .iter()
        .filter(|plugin| {
            plugin.lock().expect("Cannot acquire lock").can_teltonika_connect(&imei)
        })
        .collect(); // Collecting since i don't want to filter every time i loop over the plugins

    if allowed_plugins.is_empty() {
        log::debug!(
            target: &format!("{} {imei:15} ({peer_addr})", module_path!()),
            "No plugins allow {imei:15} to connect, sending IMEI Denial"
        );
        stream.write_imei_denial()?;
        stream
            .inner()
            .shutdown(std::net::Shutdown::Both)
            .expect("Cannot shutdown connection");

        return Ok(()); // Connection handled successfully
    }

    log::debug!(
        target: &format!("{} {imei:15} ({peer_addr})", module_path!()),
        "Connection accepted, sending IMEI Approval"
    );

    stream.write_imei_approval()?;

    for plugin in allowed_plugins.iter() {
        plugin.lock().expect("Cannot acquire lock").on_teltonika_connected(&imei);
    }

    // Last frame to check if the frame is a duplicate (eg. sent again before receiving ACK)
    let mut last_frame: Option<AVLFrame> = None;

    // Infinitely read frames
    'frame_loop: loop {
        let frame = match stream.read_frame() {
            Ok(frame) => {
                stream.write_frame_ack(Some(&frame))?;

                frame
            }
            Err(e) => match e.kind() {
                io::ErrorKind::InvalidData => {
                    log::error!(
                        target: &format!("{} {imei}", module_path!()),
                        "Error parsing frame, sending NACK",
                    );

                    stream.write_frame_ack(None)?;

                    continue;
                }
                io::ErrorKind::ConnectionReset => {
                    log::debug!(
                        target: &format!("{} {imei}", module_path!()),
                        "Connection reset by peer, closing connection"
                    );

                    break;
                }
                _ => {
                    return Err(e);
                }
            },
        };

        // Check if the frame is a duplicate
        if let Some(last_frame) = &last_frame {
            // should be different every frame, i think i will hardly find two frames with the same crc one after the other
            if last_frame.crc16 == frame.crc16
                && last_frame.records[0].timestamp == frame.records[0].timestamp
            {
                log::debug!(
                    target: &format!("{} {imei}", module_path!()),
                    "Frame is a duplicate, ignoring"
                );
                continue 'frame_loop;
            }
        }
        last_frame = Some(frame.clone());

        let event: TeltonikaEvent = frame.into();

        for plugin in allowed_plugins.iter() {
            plugin.lock().expect("Cannot acquire lock").on_teltonika_event(&imei, &event);
        }
    }

    for plugin in allowed_plugins.iter() {
        plugin.lock().expect("Cannot acquire lock").on_teltonika_disconnected(&imei);
    }

    Ok(())
}
