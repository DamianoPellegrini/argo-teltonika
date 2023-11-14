use nom_teltonika::{AVLDatagram, AVLFrame, AVLRecord, Codec};

#[derive(Debug, Clone)]
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

/// A trait for listening to events from the server
///
/// ### Thread-safety
///
/// The internal [`Vec`] of [`Plugin`]s is wrapped in an [`std::sync::Arc`] and an [`std::sync::RwLock`], so it can be
/// possible to add plugins while the server is running.
/// Each [`Plugin`] will be modified by atmost one thread, thanks to the use of [`std::sync::Mutex`]
///
/// _See the [`PluginHandle`](crate::PluginHandle) and [`PluginHandles`](crate::PluginHandles) for more info._
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
