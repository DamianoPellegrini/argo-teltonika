use std::{
    cell::RefCell,
    io,
    net::{self, SocketAddr, TcpStream},
    sync::Arc,
};

pub use nom_teltonika::{
    AVLDatagram, AVLEventIO, AVLEventIOValue, AVLPacket, AVLRecord, Codec, EventGenerationCause,
    Priority,
};

/// An event from a Teltonika device
pub struct TeltonikaEvent {
    /// The codec by the device
    pub codec: Codec,
    /// The records sent by the device
    pub records: Vec<AVLRecord>,
}

impl From<AVLPacket> for TeltonikaEvent {
    fn from(value: AVLPacket) -> Self {
        Self {
            codec: value.codec,
            records: value.records,
        }
    }
}

impl From<&AVLPacket> for TeltonikaEvent {
    fn from(value: &AVLPacket) -> Self {
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

/// A trait for listening to events from the server
pub trait Plugin {
    #[allow(unused_variables)]
    /// Check if a device is allowed to connect
    fn can_teltonika_connect(&mut self, imei: &str) -> bool {
        true
    }

    /// Notify that a new device has connected
    fn on_teltonika_connected(&mut self, imei: &str);

    /// Notify that a device has disconnected
    fn on_teltonika_disconnected(&mut self, imei: &str);

    /// Notify that a device has sent an event
    fn on_teltonika_event(&mut self, imei: &str, event: &TeltonikaEvent);
}

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn on_teltonika_connected(&mut self, imei: &str) {
        log::debug!(
            target: &format!("{} {imei:15}", module_path!()),
            "connected"
        );
    }

    fn on_teltonika_disconnected(&mut self, imei: &str) {
        log::debug!(
            target: &format!("{} {imei:15}", module_path!()),
            "disconnected"
        );
    }

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
}

pub struct Argo<'a> {
    /// The socket address to listen on
    socket_addr: SocketAddr,

    /// Listeners for events
    plugins: Arc<RefCell<Vec<Box<dyn Plugin + 'a>>>>,
}

impl<'a> Argo<'a> {
    pub fn new(addr: impl net::ToSocketAddrs) -> Self {
        Self {
            socket_addr: addr.to_socket_addrs().unwrap().next().unwrap(),
            plugins: Arc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn add_plugin<P>(&mut self, plugin: P)
    where
        P: Plugin + 'a,
    {
        self.plugins.borrow_mut().push(Box::new(plugin));
    }

    pub fn run(&self) -> io::Result<()> {
        let listener = net::TcpListener::bind(self.socket_addr)?;

        log::info!("Listening on {}", self.socket_addr);

        for client_stream in listener.incoming() {
            match client_stream {
                Ok(client) => self.handle_connection(client)?,
                Err(e) => {
                    log::error!("Client connection error: {}", e);
                }
            }
        }

        Ok(())
    }

    fn handle_connection(&self, stream: TcpStream) -> io::Result<()> {
        let mut teltonika_stream = nom_teltonika::TeltonikaStream::new(
            stream.try_clone().expect("No more handles available"),
        );
        let peer_addr = stream.peer_addr()?;
        log::debug!(
            target: &format!("{} {peer_addr}", module_path!()),
            "Connection received"
        );

        let imei = match teltonika_stream.read_imei() {
            Ok(imei) => imei,
            Err(e) => match e.kind() {
                io::ErrorKind::InvalidData => {
                    log::error!(
                        target: &format!("{} {peer_addr}", module_path!()),
                        "Error parsing IMEI, closing connection"
                    );

                    teltonika_stream.write_imei_denial()?;
                    stream.shutdown(net::Shutdown::Both)?;

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

        let mut plugins_ref = self.plugins.borrow_mut();
        let mut allowed_plugins: Vec<&mut Box<dyn Plugin>> = plugins_ref
            .iter_mut()
            .filter_map(|plugin| {
                if plugin.can_teltonika_connect(&imei) {
                    Some(plugin)
                } else {
                    None
                }
            })
            .collect();

        if allowed_plugins.is_empty() {
            log::debug!(
                target: &format!("{} {imei:15} ({peer_addr})", module_path!()),
                "Connection denied, sending IMEI Denial"
            );
            teltonika_stream.write_imei_denial()?;
            stream
                .shutdown(net::Shutdown::Both)
                .expect("Cannot shutdown connection");

            // TODO: Do I consider this as handled or as an error?
            return Ok(());
        }

        log::debug!(
            target: &format!("{} {imei:15} ({peer_addr})", module_path!()),
            "Connection accepted, sending IMEI Approval"
        );

        teltonika_stream.write_imei_approval()?;

        for listener in allowed_plugins.iter_mut() {
            listener.on_teltonika_connected(&imei);
        }

        // Last packet to check if the packet is a duplicate (eg. sent before receiving ACK)
        let mut last_packet: Option<AVLPacket> = None;

        // Infinitely read packets
        'packet_loop: loop {
            let packet = match teltonika_stream.read_packet() {
                Ok(packet) => {
                    teltonika_stream.write_packet_ack(Some(packet.records.len() as u32))?;

                    packet
                }
                Err(e) => match e.kind() {
                    io::ErrorKind::InvalidData => {
                        log::error!(
                            target: &format!("{} {imei}", module_path!()),
                            "Error parsing packet, sending NACK",
                        );

                        teltonika_stream.write_packet_ack(None)?;

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

            // Check if the packet is a duplicate
            if let Some(last_packet) = &last_packet {
                // should be different every packet, i think i will hardly find two packets with the same crc one after the other
                if last_packet.crc16 == packet.crc16
                    && last_packet.records[0].timestamp == packet.records[0].timestamp
                {
                    log::debug!(
                        target: &format!("{} {imei}", module_path!()),
                        "Packet is a duplicate, ignoring"
                    );
                    continue 'packet_loop;
                }
            }
            last_packet = Some(packet.clone());

            let event: TeltonikaEvent = packet.into();

            for listener in allowed_plugins.iter_mut() {
                listener.on_teltonika_event(&imei, &event);
            }
        }

        for listener in allowed_plugins.iter_mut() {
            listener.on_teltonika_disconnected(&imei);
        }

        Ok(())
    }
}
