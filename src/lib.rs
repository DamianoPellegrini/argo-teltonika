use std::{
    cell::RefCell,
    io::{self, Read, Write},
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

    fn handle_connection(&self, mut stream: TcpStream) -> io::Result<()> {
        let peer_addr = stream.peer_addr()?;
        log::debug!(
            target: &format!("{} {peer_addr}", module_path!()),
            "Connection received"
        );

        // Should be enough bytes, no need for a loop here
        let mut imei_buffer = [0u8; 128];
        let bytes_read = stream
            .read(&mut imei_buffer)
            .expect("Cannot read to IMEI buffer");

        if bytes_read == 0 {
            log::debug!(
                target: &format!("{} {peer_addr}", module_path!()),
                "Connection closed while reading IMEI"
            );

            // TODO: Do I consider this as handled or as an error?
            return Ok(());
        }

        let (_, imei) = nom_teltonika::parser::imei(&imei_buffer)
            .expect("Error parsing IMEI, should shutdown connection");

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
            stream.write_all(b"\x00").expect("Cannot write IMEI Denial");
            stream.flush().expect("Cannot flush stream");
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

        stream
            .write_all(b"\x01")
            .expect("Cannot write IMEI Approval");
        stream.flush().expect("Cannot flush stream");

        for listener in allowed_plugins.iter_mut() {
            listener.on_teltonika_connected(&imei);
        }

        // Last packet to check if the packet is a duplicate (eg. sent before receiving ACK)
        let mut last_packet: Option<AVLPacket> = None;

        // Infinitely read packets
        'packet_loop: loop {
            let mut parsing_buffer: Vec<u8> = Vec::with_capacity(1024);

            // Read bytes until they are enough
            'reader_loop: loop {
                let mut incoming_buffer = [0u8; 1024];
                let bytes_read = stream
                    .read(&mut incoming_buffer)
                    .expect("Cannot read to incoming buffer");

                if bytes_read == 0 {
                    log::debug!(
                        target: &format!("{} {imei}", module_path!()),
                        "Connection closed while reading packet"
                    );
                    break 'packet_loop;
                }

                parsing_buffer.extend_from_slice(&incoming_buffer);

                let packet_parser_result = nom_teltonika::parser::tcp_packet(&parsing_buffer[..]);

                // TODO: Add a timeout/retry system here
                match packet_parser_result {
                    Err(nom::Err::Incomplete(_)) => {
                        continue 'reader_loop;
                    }
                    Err(nom::Err::Error(_) | nom::Err::Failure(_)) => {
                        // Error parsing, should ask for a resend
                        log::error!(
                            target: &format!("{} {imei}", module_path!()),
                            "Error parsing packet, sending NACK",
                        );
                        stream
                            .write_all(&0_u32.to_be_bytes())
                            .expect("Cannot write packet NACK");
                        stream.flush().expect("Cannot flush stream");

                        continue 'packet_loop;
                        // break 'reader_loop;
                    }
                    Ok((_, packet)) => {
                        // Send record len as ACK
                        stream
                            .write_all(&(packet.records.len() as u32).to_be_bytes())
                            .expect("Cannot write packet ACK");
                        stream.flush().expect("Cannot flush stream");

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
                                // break 'reader_loop;
                            }
                        }
                        last_packet = Some(packet.clone());

                        for (idx, record) in packet.records.iter().enumerate() {
                            log::debug!(
                                target: &format!("{} {imei}", module_path!()),
                                "#{idx} Record timestamp: {timestamp:?}",
                                idx = idx + 1,
                                timestamp = record.timestamp
                            );
                        }

                        let event: TeltonikaEvent = packet.into();

                        for listener in allowed_plugins.iter_mut() {
                            listener.on_teltonika_event(&imei, &event);
                        }

                        break 'reader_loop;
                    }
                }
            }
        }

        for listener in allowed_plugins.iter_mut() {
            listener.on_teltonika_disconnected(&imei);
        }

        Ok(())
    }
}
