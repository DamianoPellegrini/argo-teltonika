use core::panic;

pub fn get_imei_buffer(imei: &str) -> Vec<u8> {
    let length = imei.len();
    let u16size = std::mem::size_of::<u16>();
    let size = length + u16size;
    let mut imei_buffer = vec![0; size];
    imei_buffer[0..u16size].copy_from_slice(&(length as u16).to_be_bytes());
    imei_buffer[u16size..size].copy_from_slice(imei.as_bytes());
    imei_buffer
}

pub fn get_event_id_size(codec: &nom_teltonika::Codec) -> usize {
    match codec {
        nom_teltonika::Codec::C8 => std::mem::size_of::<u8>(),
        nom_teltonika::Codec::C8Ext => std::mem::size_of::<u16>(),
        nom_teltonika::Codec::C16 => std::mem::size_of::<u16>(),
        _ => panic!("Unsupported codec: {:?}", codec),
    }
}

pub fn get_event_id_buffer(codec: &nom_teltonika::Codec, event_id: u16) -> Vec<u8> {
    match codec {
        nom_teltonika::Codec::C8 => vec![event_id as u8],
        nom_teltonika::Codec::C8Ext => event_id.to_be_bytes().to_vec(),
        nom_teltonika::Codec::C16 => event_id.to_be_bytes().to_vec(),
        _ => panic!("Unsupported codec: {:?}", codec),
    }
}

pub fn get_event_count_size(codec: &nom_teltonika::Codec) -> usize {
    match codec {
        nom_teltonika::Codec::C8 => std::mem::size_of::<u8>(),
        nom_teltonika::Codec::C8Ext => std::mem::size_of::<u16>(),
        nom_teltonika::Codec::C16 => std::mem::size_of::<u8>(),
        _ => panic!("Unsupported codec: {:?}", codec),
    }
}

pub fn get_event_count_buffer(codec: &nom_teltonika::Codec, event_count: u16) -> Vec<u8> {
    match codec {
        nom_teltonika::Codec::C8 => vec![event_count as u8],
        nom_teltonika::Codec::C8Ext => event_count.to_be_bytes().to_vec(),
        nom_teltonika::Codec::C16 => vec![event_count as u8],
        _ => panic!("Unsupported codec: {:?}", codec),
    }
}

pub fn get_packet_buffer(packet: &nom_teltonika::AVLPacket) -> Vec<u8> {
    let data_size: usize = std::mem::size_of::<nom_teltonika::Codec>() + // codec
    std::mem::size_of::<u8>() + // number of elements

    packet.records.iter().map(|r| -> usize {
        std::mem::size_of::<u64>() + // timestamp
        std::mem::size_of::<nom_teltonika::Priority>() + // priority
        std::mem::size_of::<u32>() + // longitude
        std::mem::size_of::<u32>() + // latitude
        std::mem::size_of::<u16>() + // altitude
        std::mem::size_of::<u16>() + // angle
        std::mem::size_of::<u8>() + // satellites
        std::mem::size_of::<u16>() + // speed

        get_event_id_size(&packet.codec) + // trigger event id
        if packet.codec == nom_teltonika::Codec::C16 {
            std::mem::size_of::<nom_teltonika::EventGenerationCause>()
        } else {
            0
        } + // generation type

        get_event_count_size(&packet.codec) + // number of io events
        get_event_count_size(&packet.codec) + // number of u8 io events
        get_event_count_size(&packet.codec) + // number of u16 io events
        get_event_count_size(&packet.codec) + // number of u32 io events
        get_event_count_size(&packet.codec) + // number of u64 io events
        if packet.codec == nom_teltonika::Codec::C16 {
            get_event_count_size(&packet.codec) // number of x bytes io events
        } else {
            0
        } +

        r.io_events.iter().map(|io| {
            get_event_id_size(&packet.codec) + // io event id
            match &io.value {
                nom_teltonika::AVLEventIOValue::U8(_) => std::mem::size_of::<u8>(),
                nom_teltonika::AVLEventIOValue::U16(_) => std::mem::size_of::<u16>(),
                nom_teltonika::AVLEventIOValue::U32(_) => std::mem::size_of::<u32>(),
                nom_teltonika::AVLEventIOValue::U64(_) => std::mem::size_of::<u64>(),
                nom_teltonika::AVLEventIOValue::Variable(buf) => buf.len(),
            }
        }).sum::<usize>() // io events lenght
    }).sum::<usize>() + // records

    std::mem::size_of::<u8>(); // number of elements 2
    let header_size = std::mem::size_of::<u32>() + // preamble
    std::mem::size_of::<u32>(); // data lenght
    let buffer_size = header_size + data_size + std::mem::size_of::<u32>(); // crc 16

    let mut packet_buffer = Vec::with_capacity(buffer_size);
    packet_buffer.extend([0x00, 0x00, 0x00, 0x00].iter()); // preamble
    packet_buffer.extend((data_size as u32).to_be_bytes().iter()); // data lenght
    packet_buffer.push(packet.codec.into()); // codec
    packet_buffer.push(packet.records.len() as u8); // number of elements
    packet_buffer.extend(packet.records.iter().flat_map(|r| {
        let mut record_buffer = Vec::with_capacity(data_size);
        record_buffer.extend((r.timestamp.timestamp_millis()).to_be_bytes().iter()); // timestamp
        record_buffer.push(r.priority as u8); // priority
        record_buffer.extend(((r.longitude * 10000000.0) as u32).to_be_bytes().iter()); // longitude
        record_buffer.extend(((r.latitude * 10000000.0) as u32).to_be_bytes().iter()); // latitude
        record_buffer.extend((r.altitude).to_be_bytes().iter()); // altitude
        record_buffer.extend((r.angle).to_be_bytes().iter()); // angle
        record_buffer.push(r.satellites); // satellites
        record_buffer.extend((r.speed).to_be_bytes().iter()); // speed
        record_buffer.extend(get_event_id_buffer(&packet.codec, r.trigger_event_id).iter()); // trigger event id
        if let Some(generation_type) = r.generation_type {
            record_buffer.push(generation_type as u8); // generation type
        }
        record_buffer
            .extend(get_event_count_buffer(&packet.codec, r.io_events.len() as u16).iter()); // number of io events
        let u8_events = r
            .io_events
            .iter()
            .filter(|io| matches!(io.value, nom_teltonika::AVLEventIOValue::U8(_)));
        let u16_events = r
            .io_events
            .iter()
            .filter(|io| matches!(io.value, nom_teltonika::AVLEventIOValue::U16(_)));
        let u32_events = r
            .io_events
            .iter()
            .filter(|io| matches!(io.value, nom_teltonika::AVLEventIOValue::U32(_)));
        let u64_events = r
            .io_events
            .iter()
            .filter(|io| matches!(io.value, nom_teltonika::AVLEventIOValue::U64(_)));
        record_buffer
            .extend(get_event_count_buffer(&packet.codec, u8_events.clone().count() as u16).iter()); // number of u8 io events
        record_buffer.extend(u8_events.flat_map(|io| {
            let mut io_buffer = Vec::new();
            io_buffer.extend(get_event_id_buffer(&packet.codec, io.id).iter());
            io_buffer.extend(match &io.value {
                nom_teltonika::AVLEventIOValue::U8(v) => v.to_be_bytes().to_vec(),
                _ => unreachable!(),
            });
            io_buffer
        })); // u8 io events

        record_buffer.extend(
            get_event_count_buffer(&packet.codec, u16_events.clone().count() as u16).iter(),
        ); // number of u16 io events
        record_buffer.extend(u16_events.flat_map(|io| {
            let mut io_buffer = Vec::new();
            io_buffer.extend(get_event_id_buffer(&packet.codec, io.id).iter());
            io_buffer.extend(match &io.value {
                nom_teltonika::AVLEventIOValue::U16(v) => v.to_be_bytes().to_vec(),
                _ => unreachable!(),
            });
            io_buffer
        })); // u16 io events

        record_buffer.extend(
            get_event_count_buffer(&packet.codec, u32_events.clone().count() as u16).iter(),
        ); // number of u32 io events
        record_buffer.extend(u32_events.flat_map(|io| {
            let mut io_buffer = Vec::new();
            io_buffer.extend(get_event_id_buffer(&packet.codec, io.id).iter());
            io_buffer.extend(match &io.value {
                nom_teltonika::AVLEventIOValue::U32(v) => v.to_be_bytes().to_vec(),
                _ => unreachable!(),
            });
            io_buffer
        })); // u32 io events

        record_buffer.extend(
            get_event_count_buffer(&packet.codec, u64_events.clone().count() as u16).iter(),
        ); // number of u64 io events
        record_buffer.extend(u64_events.flat_map(|io| {
            let mut io_buffer = Vec::new();
            io_buffer.extend(get_event_id_buffer(&packet.codec, io.id).iter());
            io_buffer.extend(match &io.value {
                nom_teltonika::AVLEventIOValue::U64(v) => v.to_be_bytes().to_vec(),
                _ => unreachable!(),
            });
            io_buffer
        })); // u64 io events

        if packet.codec == nom_teltonika::Codec::C8Ext {
            let xb_events = r
                .io_events
                .iter()
                .filter(|io| matches!(io.value, nom_teltonika::AVLEventIOValue::Variable(_)));
            record_buffer.extend(
                get_event_count_buffer(&packet.codec, xb_events.clone().count() as u16).iter(),
            ); // number of x bytes io events
            record_buffer.extend(xb_events.flat_map(|io| {
                let mut io_buffer: Vec<u8> = Vec::new();
                io_buffer.extend(get_event_id_buffer(&packet.codec, io.id).iter());
                io_buffer.extend(match &io.value {
                    nom_teltonika::AVLEventIOValue::Variable(v) => v,
                    _ => unreachable!(),
                });
                io_buffer
            })); // xb io events
        }
        record_buffer
    })); // records
    packet_buffer.push(packet.records.len() as u8); // number of elements
    packet_buffer.extend(
        (nom_teltonika::crc16(&packet_buffer[header_size..]) as u32)
            .to_be_bytes()
            .iter(),
    ); // crc 16
    packet_buffer
}

#[cfg(test)]
mod tests {
    use nom_teltonika::{AVLEventIO, AVLRecord};

    use super::*;

    #[test]
    fn test_get_imei_buffer() {
        let imei_buffer = get_imei_buffer("123456789012345");
        let (_, parsed) = nom_teltonika::parser::imei(&imei_buffer).unwrap();
        assert_eq!(parsed, "123456789012345");
    }

    #[test]
    fn test_get_packet_buffer() {
        let mut packet = nom_teltonika::AVLPacket {
            codec: nom_teltonika::Codec::C8,
            records: vec![AVLRecord {
                timestamp: chrono::TimeZone::timestamp_millis_opt(
                    &chrono::Utc,
                    chrono::Utc::now().timestamp_millis(),
                )
                .single()
                .unwrap(),
                priority: nom_teltonika::Priority::High,
                longitude: 0.0,
                latitude: 0.0,
                altitude: 0,
                angle: 0,
                satellites: 0,
                speed: 0,
                trigger_event_id: 1,
                generation_type: None,
                io_events: vec![
                    AVLEventIO {
                        id: 21,
                        value: nom_teltonika::AVLEventIOValue::U8(3),
                    },
                    AVLEventIO {
                        id: 1,
                        value: nom_teltonika::AVLEventIOValue::U8(1),
                    },
                    AVLEventIO {
                        id: 66,
                        value: nom_teltonika::AVLEventIOValue::U16(24079),
                    },
                    AVLEventIO {
                        id: 241,
                        value: nom_teltonika::AVLEventIOValue::U32(24602),
                    },
                    AVLEventIO {
                        id: 78,
                        value: nom_teltonika::AVLEventIOValue::U64(0),
                    },
                ],
            }],
            crc16: 51151,
        };
        let packet_buffer = get_packet_buffer(&packet);
        packet.crc16 = nom_teltonika::crc16(&packet_buffer[8..&packet_buffer.len() - 4]) as u32;
        let (_, parsed) = nom_teltonika::parser::tcp_packet(&packet_buffer).unwrap();
        assert_eq!(parsed, packet);
    }
}
