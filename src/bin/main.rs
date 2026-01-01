#![no_std]
#![no_main]

use esp_blinky_rust::{setup, Duration, Timer};
use esp_blinky_rust::config::ConfigStore;
use rtt_target::rprintln;
use embassy_executor::Spawner;
use esp_radio::wifi::{ClientConfig, ModeConfig, WifiDevice};
use embassy_net::{Runner, Config as NetConfig, StackResources, Ipv4Address};
use embassy_net::tcp::TcpSocket;
use embedded_io_async::{Read, Write}; // Required for socket.read/write
use static_cell::StaticCell;
use heapless::String;
use core::str::FromStr;
use alloc::string::ToString;

extern crate alloc;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    rprintln!("PANIC: {:?}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

/// Background task to drive the network stack.
/// This task runs the background network operations (DHCP, TCP/IP state machine, etc.).
/// It must be spawned for the stack to function.
#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

// --- Simple MQTT Helper Functions ---

/// Helper to send an MQTT CONNECT packet and wait for CONNACK.
/// This is a minimal implementation to avoid dependency issues with complex MQTT crates.
async fn mqtt_connect<'a>(socket: &mut TcpSocket<'a>, client_id: &str) -> Result<(), ()> {
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
async fn mqtt_publish<'a>(socket: &mut TcpSocket<'a>, topic: &str, payload: &[u8]) -> Result<(), ()> {
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

// --- Main Application ---

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_print!();
    
    rprintln!("Initializing...");
    let mut app = setup(spawner).await;

    // 1. Load Configuration
    // We load Wi-Fi credentials and MQTT settings from Flash memory.
    let mut config_store = ConfigStore::new(app.flash);
    let config = config_store.load().await.unwrap_or_default();

    rprintln!("Booting... SSID='{}'", config.ssid);

    // 2. Configure Wi-Fi
    // We use the credentials from the config store.
    let client_config = ClientConfig::default();
    let client_config = client_config.with_ssid(config.ssid.to_string());
    let client_config = client_config.with_password(config.password.to_string());
    
    if let Err(e) = app.wifi.set_config(&ModeConfig::Client(client_config)) {
        rprintln!("Error setting Wi-Fi config: {:?}", e);
    }

    // 3. Connect to Wi-Fi
    // We attempt to connect in a loop until successful.
    rprintln!("Connecting to Wi-Fi...");
    loop {
        // Use connect_async() to await the connection process
        match app.wifi.connect_async().await {
            Ok(_) => {
                rprintln!("Wi-Fi Connected!");
                break;
            }
            Err(e) => {
                rprintln!("Wi-Fi Connect Failed: {:?}. Retrying in 3s...", e);
                Timer::after(Duration::from_millis(3000)).await;
            }
        }
    }

    // 4. Initialize Network Stack
    // We allocate static resources for the embassy-net stack.
    // Use StackResources<3> for 3 sockets.
    static STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

    // Initialize the stack (embassy_net::new returns stack handle + runner)
    // We pass app.wifi_interface directly (by value), so the Runner takes ownership of it.
    let (stack, runner) = embassy_net::new(
        app.wifi_interface,
        NetConfig::dhcpv4(Default::default()),
        STACK_RESOURCES.init(StackResources::<3>::new()),
        1234, // Random seed (Replace with TRNG for production security)
    );

    // Start the background network task
    spawner.spawn(net_task(runner)).unwrap();

    // Wait for DHCP to acquire an IP address
    rprintln!("Waiting for IP address...");
    loop {
        if stack.is_config_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
    
    if let Some(config) = stack.config_v4() {
        rprintln!("Network Up! IP: {:?}", config.address);
    }

    // 5. MQTT Configuration
    let mut rx_buffer = [0u8; 1024];
    let mut tx_buffer = [0u8; 1024];

    // Parse the MQTT broker IP from the config string
    let broker_ip: Ipv4Address = match Ipv4Address::from_str(config.mqtt_host.as_str()) {
        Ok(ip) => ip,
        Err(_) => {
            rprintln!("Error: MQTT Host '{}' is not a valid IPv4 address.", config.mqtt_host);
            Ipv4Address::new(127, 0, 0, 1) // Fallback to localhost
        }
    };
    
    let broker_endpoint = (broker_ip, config.mqtt_port);

    // 6. Main Application Loop
    // Connects to MQTT, publishes temperature, and handles reconnections.
    loop {
        rprintln!("Connecting to MQTT Broker at {:?}:{}...", broker_endpoint.0, broker_endpoint.1);
        
        // Create a TCP socket
        // 'stack' is a Copy handle, so we pass it directly
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        // TCP Connect
        if let Err(e) = socket.connect(broker_endpoint).await {
            rprintln!("TCP Connect failed: {:?}. Retrying in 5s...", e);
            Timer::after(Duration::from_secs(5)).await;
            continue;
        }

        rprintln!("TCP Connected. Sending MQTT CONNECT...");
        // MQTT Handshake
        if let Err(_) = mqtt_connect(&mut socket, config.device_id.as_str()).await {
             rprintln!("MQTT CONNECT failed. Closing socket.");
             socket.close();
             Timer::after(Duration::from_secs(5)).await;
             continue;
        }

        rprintln!("MQTT Connected! Starting publish loop...");
        
        // Publish Loop
        loop {
            // Read Temperature
            let temp = app.temp_sensor.get_temperature().to_celsius();
            rprintln!("Status: Running | Temp: {:.1} C", temp);
             
            // Format Payload
            let mut payload = String::<64>::new();
            use core::fmt::Write;
            if write!(payload, "{:.1}", temp).is_ok() {
                // Publish
                if let Err(_) = mqtt_publish(&mut socket, "sensors/temp", payload.as_bytes()).await {
                    rprintln!("Publish failed. Reconnecting...");
                    break; // Break inner loop to trigger reconnection
                }
                rprintln!("Published: sensors/temp -> {}", payload);
            }

            // Blink LED
            app.led.toggle();
            
            // Sleep before next publish
            Timer::after(Duration::from_secs(2)).await;
        }
        
        // Cleanup before retrying
        socket.close();
        Timer::after(Duration::from_secs(5)).await;
    }
}