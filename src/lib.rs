use std::{
    io,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
};

use nom_teltonika::{AVLDatagram, AVLFrame, AVLRecord, Codec};

// TODO: async can connect in filter

/// An event from a Teltonika device
pub struct TeltonikaEvent {
    /// The codec by the device
    pub codec: Codec,
    /// The records sent by the device
    pub records: Vec<AVLRecord>,
}

impl From<AVLFrame> for TeltonikaEvent {
    fn from(value: AVLFrame) -> Self {
        Self {
            codec: value.codec,
            records: value.records,
        }
    }
}

impl From<&AVLFrame> for TeltonikaEvent {
    fn from(value: &AVLFrame) -> Self {
        Self {
            codec: value.codec,
            records: value.records.clone(),
        }
    }
}

impl From<AVLDatagram> for TeltonikaEvent {
    fn from(value: AVLDatagram) -> Self {
        Self {
            codec: value.codec,
            records: value.records,
        }
    }
}

impl From<&AVLDatagram> for TeltonikaEvent {
    fn from(value: &AVLDatagram) -> Self {
        Self {
            codec: value.codec,
            records: value.records.clone(),
        }
    }
}

#[cfg_attr(feature = "async", async_trait::async_trait)]
/// A trait for listening to events from the server
/// # Thread-safety
/// Plugins are called concurrently by every module in a mutually exclusive way,
/// so it is safe to modify state in a plugin without using locks or synchronization mechanisms.
/// See the [`PluginHandle`] and [`PluginHandles`] signatures to understand how it is implemented.
pub trait Plugin {
    #[allow(unused_variables)]
    /// Check if a device is allowed to connect
    fn can_teltonika_connect(&mut self, imei: &str) -> bool {
        true
    }
    
    #[cfg(not(feature = "async"))]
    /// Notify that a new device has connected
    fn on_teltonika_connected(&mut self, imei: &str);
    
    #[cfg(feature = "async")]
    /// Notify that a new device has connected
    async fn on_teltonika_connected(&mut self, imei: &str);

    #[cfg(not(feature = "async"))]
    /// Notify that a device has disconnected
    fn on_teltonika_disconnected(&mut self, imei: &str);
    

    #[cfg(feature = "async")]
    /// Notify that a device has disconnected
    async fn on_teltonika_disconnected(&mut self, imei: &str);

    #[cfg(not(feature = "async"))]
    /// Notify that a device has sent an event
    fn on_teltonika_event(&mut self, imei: &str, event: &TeltonikaEvent);


    #[cfg(feature = "async")]
    /// Notify that a device has sent an event
    async fn on_teltonika_event(&mut self, imei: &str, event: &TeltonikaEvent);
}

pub struct DebugPlugin;

#[cfg_attr(feature = "async", async_trait::async_trait)]
impl Plugin for DebugPlugin {
    #[cfg(not(feature = "async"))]
    fn on_teltonika_connected(&mut self, imei: &str) {
        log::debug!(
            target: &format!("{} {imei:15}", module_path!()),
            "connected"
        );
    }

    #[cfg(feature = "async")]
    async fn on_teltonika_connected(&mut self, imei: &str) {
        log::debug!(
            target: &format!("{} {imei:15}", module_path!()),
            "connected"
        );
    } 

    #[cfg(not(feature = "async"))]
    fn on_teltonika_disconnected(&mut self, imei: &str) {
        log::debug!(
            target: &format!("{} {imei:15}", module_path!()),
            "disconnected"
        );
    }

    #[cfg(feature = "async")]
    async fn on_teltonika_disconnected(&mut self, imei: &str) {
        log::debug!(
            target: &format!("{} {imei:15}", module_path!()),
            "disconnected"
        );
    }

    #[cfg(not(feature = "async"))]
    fn on_teltonika_event(&mut self, imei: &str, event: &TeltonikaEvent) {
        let mut event_str = format!("Sent an event:");

        for record in event.records.iter() {
            event_str.push_str(&format!(
                "\r\n{:>10}- Position: {2}, {3}, {4} at {1}",
                "", record.timestamp, record.latitude, record.longitude, record.altitude
            ));
        }
        log::debug!(
            target: &format!("{} {imei:15}", module_path!()),
            "{event_str}"
        );
    }

    #[cfg(feature = "async")]
    async fn on_teltonika_event(&mut self, imei: &str, event: &TeltonikaEvent) {
        let mut event_str = format!("Sent an event:");

        for record in event.records.iter() {
            event_str.push_str(&format!(
                "\r\n{:>10}- Position: {2}, {3}, {4} at {1}",
                "", record.timestamp, record.latitude, record.longitude, record.altitude
            ));
        }
        log::debug!(
            target: &format!("{} {imei:15}", module_path!()),
            "{event_str}"
        );
    }
}

type PluginHandle = Arc<Mutex<dyn Plugin + Send + Sync>>;
type PluginHandles = Arc<RwLock<Vec<PluginHandle>>>;

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
            .push(Arc::new(Mutex::new(plugin)));
    }

    #[cfg(not(feature = "async"))]
    pub fn run(&self) -> io::Result<()> {
        let listener = std::net::TcpListener::bind(self.socket_addr)?;

        log::info!("Listening on {}", self.socket_addr);

        // // Here HANDLES remain in memory, find another solution cause it uses too much memory
        // let mut handles = vec![];

        for client_stream in listener.incoming() {
            match client_stream {
                Ok(client) => {
                    let peer_addr = client.peer_addr()?;
                    log::debug!(
                        target: &format!("{} {peer_addr}", module_path!()),
                        "Connection received"
                    );

                    // Clone a ref and lock as readable
                    let plugins: PluginHandles = self.plugins.clone();

                    std::thread::spawn(move || handle_connection(client, plugins, peer_addr));

                    // handles.push(handle);

                    // handles.retain(|handle| !handle.is_finished());
                    // handles.shrink_to_fit();
                }
                Err(e) => {
                    log::error!("Client connection error: {}", e);
                    continue;
                }
            }
        }

        // for handle in handles.into_iter() {
        //     match handle.join() {
        //         Ok(result) => {
        //             if let Err(e) = result {
        //                 log::error!("Error on connection thread: {}", e);
        //             }
        //         }
        //         Err(e) => {
        //             log::error!("Error on connection thread: {:?}", e);
        //         }
        //     }
        // }

        Ok(())
    }
}

#[cfg(not(feature = "async"))]
fn handle_connection(
    stream: std::net::TcpStream,
    plugins: PluginHandles,
    peer_addr: std::net::SocketAddr,
) -> io::Result<()> {
    // Clone every plugin ref to be locked
    // Each plugin can be locked by the individual threads
    let plugins = plugins
        .read()
        .unwrap()
        .iter()
        .map(|plugin| plugin.clone())
        .collect::<Vec<_>>();
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

    let mut allowed_plugins = plugins
        .iter()
        .filter_map(|plugin| {
            if plugin.lock().unwrap().can_teltonika_connect(&imei) {
                Some(plugin)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if allowed_plugins.is_empty() {
        log::debug!(
            target: &format!("{} {imei:15} ({peer_addr})", module_path!()),
            "Connection denied, sending IMEI Denial"
        );
        stream.write_imei_denial()?;
        stream
            .inner()
            .shutdown(std::net::Shutdown::Both)
            .expect("Cannot shutdown connection");

        // TODO: Do I consider this as handled or as an error?
        return Ok(());
    }

    log::debug!(
        target: &format!("{} {imei:15} ({peer_addr})", module_path!()),
        "Connection accepted, sending IMEI Approval"
    );

    stream.write_imei_approval()?;

    for plugin in allowed_plugins.iter_mut() {
        plugin.lock().unwrap().on_teltonika_connected(&imei);
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

        for plugin in allowed_plugins.iter_mut() {
            plugin.lock().unwrap().on_teltonika_event(&imei, &event);
        }
    }

    for plugin in allowed_plugins.iter_mut() {
        plugin.lock().unwrap().on_teltonika_disconnected(&imei);
    }

    Ok(())
}

#[cfg(feature = "async")]
impl Argo {
    pub async fn run(&self) -> io::Result<()> {
        let listener = tokio::net::TcpListener::bind(self.socket_addr).await?;

        log::info!("Listening on {}", self.socket_addr);

        while let Ok((stream, peer_addr)) = listener.accept().await {
            log::debug!(
                target: &format!("{} {peer_addr}", module_path!()),
                "Connection received"
            );
            tokio::task::spawn(handle_connection(stream, self.plugins.clone(), peer_addr));
        }

        Ok(())
    }
}

#[cfg(feature = "async")]
async fn handle_connection(
    stream: tokio::net::TcpStream,
    plugins: PluginHandles,
    peer_addr: std::net::SocketAddr,
) -> io::Result<()> {
    // Clone every plugin ref to be locked
    // Each plugin can be locked by the individual threads

    use tokio::io::AsyncWriteExt;
    let plugins = plugins
        .read()
        .unwrap()
        .iter()
        .map(|plugin| plugin.clone())
        .collect::<Vec<_>>();
    let mut stream = nom_teltonika::TeltonikaStream::new(stream);

    let imei = match stream.read_imei_async().await {
        Ok(imei) => imei,
        Err(e) => match e.kind() {
            io::ErrorKind::InvalidData => {
                log::error!(
                    target: &format!("{} {peer_addr}", module_path!()),
                    "Error parsing IMEI, closing connection"
                );

                stream.write_imei_denial_async().await?;
                stream.inner_mut().shutdown().await?;

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

    let mut allowed_plugins = plugins
        .iter()
        .filter_map(|plugin| {


            if plugin.lock().unwrap().can_teltonika_connect(&imei) {
                Some(plugin)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if allowed_plugins.is_empty() {
        log::debug!(
            target: &format!("{} {imei:15} ({peer_addr})", module_path!()),
            "Connection denied, sending IMEI Denial"
        );
        stream.write_imei_denial_async().await?;
        stream
            .inner_mut()
            .shutdown()
            .await
            .expect("Cannot shutdown connection");

        // TODO: Do I consider this as handled or as an error?
        return Ok(());
    }

    log::debug!(
        target: &format!("{} {imei:15} ({peer_addr})", module_path!()),
        "Connection accepted, sending IMEI Approval"
    );

    stream.write_imei_approval_async().await?;

    for plugin in allowed_plugins.iter_mut() {
        plugin.lock().unwrap().on_teltonika_connected(&imei).await;
    }

    // Last frame to check if the frame is a duplicate (eg. sent again before receiving ACK)
    let mut last_frame: Option<AVLFrame> = None;

    // Infinitely read frames
    'frame_loop: loop {
        let frame = match stream.read_frame_async().await {
            Ok(frame) => {
                stream.write_frame_ack_async(Some(&frame)).await?;

                frame
            }
            Err(e) => match e.kind() {
                io::ErrorKind::InvalidData => {
                    log::error!(
                        target: &format!("{} {imei}", module_path!()),
                        "Error parsing frame, sending NACK",
                    );

                    stream.write_frame_ack_async(None).await?;

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

        for plugin in allowed_plugins.iter_mut() {
            plugin.lock().unwrap().on_teltonika_event(&imei, &event).await;
        }
    }

    for plugin in allowed_plugins.iter_mut() {
        plugin.lock().unwrap().on_teltonika_disconnected(&imei).await;
    }

    Ok(())
}
