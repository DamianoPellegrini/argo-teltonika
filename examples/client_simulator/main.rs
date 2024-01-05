use std::{
    io::{Read, Write},
    net::TcpStream,
    process::exit,
    thread::sleep,
};

use std::time::Duration;

use nom_teltonika::{AVLEventIO, AVLPacket, AVLRecord};

mod serializer;

fn main() -> std::io::Result<()> {
    let mut connection =
        TcpStream::connect("localhost:56552").expect("Failed to connect to server");

    std::env::args().for_each(|arg| println!("{}", arg));

    let imei = match std::env::args().find(|arg| arg.starts_with("--imei")) {
        Some(arg) => arg.split('=').last().unwrap().to_owned(),
        None => "123456789012345".into(),
    };

    let imei_buf = serializer::get_imei_buffer(&imei);

    println!("Sending IMEI");
    let mut total_bytes_written = 0;
    loop {
        let bytes_written = connection.write(&imei_buf)?;
        connection.flush()?;

        if bytes_written == 0 {
            println!("Server disconnected");
            exit(1);
        }

        total_bytes_written += bytes_written;

        if total_bytes_written == imei_buf.len() {
            break;
        }
    }
    connection.flush()?;

    let mut buf = [0; 1];
    let bytes_read = connection.read(&mut buf)?;

    // if denied
    if buf[0] == 0 || bytes_read == 0 {
        println!("Denied communication by server");
        exit(1);
    }

    loop {
        println!("Generating packet...");
        let mut packet = AVLPacket {
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

        
        if chrono::Utc::now().timestamp() % 2 == 1 {
            sleep(Duration::from_millis(100));
            packet.records.push(AVLRecord {
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
                ],
            });
        }

        if chrono::Utc::now().timestamp() % 3 == 0 {
            sleep(Duration::from_millis(100));
            packet.records.push(AVLRecord {
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
                ],
            });
        }

        let packet_buf = serializer::get_packet_buffer(&packet);

        println!("Sending {} record(s)", packet.records.len());
        // Resend if not ACK'd
        loop {
            let mut total_bytes_written = 0;
            loop {
                let bytes_written = connection.write(&packet_buf)?;
                connection.flush()?;

                if bytes_written == 0 {
                    println!("Server disconnected");
                    exit(1);
                }

                total_bytes_written += bytes_written;

                if total_bytes_written == packet_buf.len() {
                    break;
                }
            }

            let mut buf = [0; 4];
            let bytes_read = connection.read(&mut buf)?;

            if bytes_read == 0 {
                println!("Server disconnected");
                exit(1);
            }

            let data_len = u32::from_be_bytes(buf);
            println!("Received ACK ({data_len} record(s))");

            // ACK
            if data_len == packet.records.len() as u32 {
                break;
            } else {
                println!("Resending packet...");
                sleep(Duration::from_millis(100));
            }
        }

        println!("Waiting...");
        sleep(Duration::from_millis(1250));
    }
}
