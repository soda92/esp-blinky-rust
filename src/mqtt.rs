use rtt_target::rprintln;
use embassy_net::tcp::TcpSocket;
use embedded_io_async::{Read, Write};

// --- Simple MQTT Helper Functions ---

/// Helper to send an MQTT CONNECT packet and wait for CONNACK.
/// This is a minimal implementation to avoid dependency issues with complex MQTT crates.
pub async fn mqtt_connect<'a>(socket: &mut TcpSocket<'a>, client_id: &str) -> Result<(), ()> {
    // Fixed Header: Type 1 (CONNECT)
    // Variable Header: Protocol Name (MQTT), Level (4), Flags (Clean Session), Keep Alive
    // Payload: Client ID
    
    let client_id_bytes = client_id.as_bytes();
    // Header overhead: Len(2) + MQTT(4) + Lvl(1) + Flags(1) + KeepAlive(2) = 10 bytes
    let var_header_len = 10; 
    let payload_len = 2 + client_id_bytes.len(); // 2 bytes for length prefix + ID bytes
    let rem_len = var_header_len + payload_len;
    
    if rem_len > 127 {
        rprintln!("MQTT Error: Connect packet too long for simple helper (max 127 bytes)");
        return Err(());
    }

    let mut packet = [0u8; 128];
    let mut idx = 0;

    // Fixed Header
    packet[idx] = 0x10; idx += 1; // Type 1 (CONNECT) | Reserved (0)
    packet[idx] = rem_len as u8; idx += 1; // Remaining Length

    // Variable Header
    // Protocol Name "MQTT"
    packet[idx] = 0x00; idx += 1;
    packet[idx] = 0x04; idx += 1;
    packet[idx] = b'M'; idx += 1;
    packet[idx] = b'Q'; idx += 1;
    packet[idx] = b'T'; idx += 1;
    packet[idx] = b'T'; idx += 1;
    // Protocol Level 4 (v3.1.1)
    packet[idx] = 0x04; idx += 1;
    // Connect Flags: Clean Session (0x02)
    packet[idx] = 0x02; idx += 1;
    // Keep Alive: 60s (0x003C)
    packet[idx] = 0x00; idx += 1;
    packet[idx] = 60; idx += 1;

    // Payload: Client ID (prefixed with length)
    let len_be = (client_id_bytes.len() as u16).to_be_bytes();
    packet[idx] = len_be[0]; idx += 1;
    packet[idx] = len_be[1]; idx += 1;
    
    // Copy Client ID bytes
    packet[idx..idx+client_id_bytes.len()].copy_from_slice(client_id_bytes);
    idx += client_id_bytes.len();

    // Send the CONNECT packet
    socket.write_all(&packet[..idx]).await.map_err(|_| ())?;

    // Receive CONNACK (4 bytes)
    // Format: 20 02 SP RC
    let mut rx = [0u8; 4];
    socket.read_exact(&mut rx).await.map_err(|_| ())?;

    // Check Connack Flags (SP) and Return Code (RC)
    // 0x20 = CONNACK packet type
    // 0x00 = Connection Accepted
    if rx[0] == 0x20 && rx[3] == 0x00 {
        Ok(())
    } else {
        rprintln!("MQTT: Connection refused. Response: {:02x?}", rx);
        Err(())
    }
}

/// Helper to send an MQTT PUBLISH packet.
/// QoS is set to 0 (At Most Once) for simplicity.
pub async fn mqtt_publish<'a>(socket: &mut TcpSocket<'a>, topic: &str, payload: &[u8]) -> Result<(), ()> {
    // Fixed Header: Type 3 (PUBLISH), QoS 0 (0x30)
    // Variable Header: Topic Name (Length + String)
    // Payload: Data

    let topic_bytes = topic.as_bytes();
    let rem_len = 2 + topic_bytes.len() + payload.len(); // 2 bytes for topic len

    if rem_len > 127 {
        rprintln!("MQTT Error: Publish packet too long");
        return Err(());
    }

    let mut header = [0u8; 128];
    let mut idx = 0;

    // Fixed Header
    header[idx] = 0x30; idx += 1; // Type 3 (PUBLISH) | QoS 0
    header[idx] = rem_len as u8; idx += 1;

    // Variable Header: Topic Name
    let tlen_be = (topic_bytes.len() as u16).to_be_bytes();
    header[idx] = tlen_be[0]; idx += 1;
    header[idx] = tlen_be[1]; idx += 1;

    header[idx..idx+topic_bytes.len()].copy_from_slice(topic_bytes);
    idx += topic_bytes.len();

    // Send Header + Topic
    socket.write_all(&header[..idx]).await.map_err(|_| ())?;

    // Send Payload
    socket.write_all(payload).await.map_err(|_| ())?;

    Ok(())
}
