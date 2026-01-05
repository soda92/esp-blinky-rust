#![no_std]
#![no_main]

use esp_blinky_rust::{setup, Duration, Timer};
use esp_blinky_rust::config::ConfigStore;
use esp_blinky_rust::mqtt::{mqtt_connect, mqtt_publish};
use rtt_target::rprintln;
use embassy_executor::Spawner;
use esp_radio::wifi::{ClientConfig, ModeConfig, WifiDevice};
use embassy_net::{Runner, Config as NetConfig, StackResources, Ipv4Address};
use embassy_net::tcp::TcpSocket;
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