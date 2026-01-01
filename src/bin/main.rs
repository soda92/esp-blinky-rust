#![no_std]
#![no_main]

use esp_blinky_rust::{setup, Duration, Timer};
use esp_blinky_rust::config::ConfigStore;
use rtt_target::rprintln;
use embassy_executor::Spawner;
use esp_radio::wifi::{ClientConfig, ModeConfig, WifiDevice};
use embassy_net::{Stack, Config as NetConfig, StackResources, Ipv4Address};
use minimq::{Minimq, Publication, QoS};
use static_cell::StaticCell;
use heapless::String;
use core::str::FromStr;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    rprintln!("PANIC: {:?}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

// Network Stack Task
#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static>>) {
    stack.run().await
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_print!();
    
    rprintln!("Initializing...");
    let mut app = setup(spawner).await;

    // Initialize Config Store
    let mut config_store = ConfigStore::new(app.flash);
    let config = config_store.load().await.unwrap_or_default();

    rprintln!("Booting... SSID='{}'", config.ssid);

    // 1. Setup WiFi Config
    let client_config = ClientConfig {
        ssid: config.ssid.clone(),
        password: config.password.clone(),
        ..Default::default()
    };
    
    if let Err(e) = app.wifi.set_config(&ModeConfig::Client(client_config)) {
        rprintln!("Error setting Wi-Fi config: {:?}", e);
    }

    // 2. Connect to WiFi
    rprintln!("Connecting to Wi-Fi...");
    loop {
        match app.wifi.connect(&esp_blinky_rust::ScanConfig::default()).await {
            Ok(_) => {
                rprintln!("Wi-Fi Connected!");
                break;
            }
            Err(e) => {
                rprintln!("Wi-Fi Connect Failed: {:?}. Checking SSID/Password and retrying...", e);
                Timer::after(Duration::from_millis(3000)).await;
            }
        }
    }

    // 3. Initialize Network Stack
    static STACK: StaticCell<Stack<WifiDevice<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

    let stack = STACK.init(Stack::new(
        app.wifi_interface,
        NetConfig::dhcpv4(Default::default()),
        RESOURCES.init(StackResources::<3>::new()),
        1234, // Random seed (should use TRNG in production)
    ));

    spawner.spawn(net_task(stack)).unwrap();

    rprintln!("Waiting for IP...");
    loop {
        if stack.is_config_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
    
    if let Some(config) = stack.config_v4() {
        rprintln!("Got IP: {:?}", config.address);
    }

    // 4. MQTT Setup
    let mut rx_buffer = [0u8; 1024];
    let mut tx_buffer = [0u8; 1024];

    // Parse IP for MQTT broker
    // config.mqtt_host is e.g., "192.168.0.107"
    let broker_ip: Ipv4Address = match Ipv4Address::from_str(config.mqtt_host.as_str()) {
        Ok(ip) => ip,
        Err(_) => {
            rprintln!("Error: MQTT Host '{}' is not a valid IPv4 address.", config.mqtt_host);
            // Default to localhost (won't work but safe fallback)
            Ipv4Address::new(127, 0, 0, 1)
        }
    };
    let broker_ip = embassy_net::IpAddress::Ipv4(broker_ip);

    rprintln!("Connecting to MQTT Broker at {:?}...", broker_ip);

    let mut mqtt: Minimq<'_, _, _, minimq::broker::IpBroker> = Minimq::new(
        stack, 
        config.device_id.as_str(), 
        minimq::ConfigBuilder::new(broker_ip.into(), &mut rx_buffer, &mut tx_buffer)
            .keepalive_interval(60)
    );

    loop {
        if !mqtt.client().is_connected() {
            // Attempt connection
            match mqtt.poll(|_client, _topic, _message, _properties| {}) {
                Ok(_) => {
                    if mqtt.client().is_connected() {
                         rprintln!("MQTT Connected!");
                    }
                }
                Err(e) => {
                    // Minimq errors can be verbose, simplified logging
                    // rprintln!("MQTT Connection Error"); 
                }
            }
        } else {
             // Read Temp
             let temp = app.temp_sensor.get_temperature().to_celsius();
             rprintln!("Status: Running | Temp: {:.1} C", temp);
             
             // Publish
             let mut payload = String::<64>::new();
             use core::fmt::Write;
             if write!(payload, "{:.1}", temp).is_ok() {
                 let topic = "sensors/temp";
                 match mqtt.client().publish(
                     Publication::new(payload.as_bytes())
                        .topic(topic)
                        .qos(QoS::AtMostOnce)
                 ) {
                     Ok(_) => rprintln!("Published: {} -> {}", topic, payload),
                     Err(e) => rprintln!("Publish Error: {:?}", e),
                 }
             }
        }

        // Poll for incoming messages/keepalive
        let _ = mqtt.poll(|_client, _topic, _message, _properties| {});

        app.led.toggle();
        Timer::after(Duration::from_secs(2)).await;
    }
}