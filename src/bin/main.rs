#![no_std]
#![no_main]

use esp_blinky_rust::{setup, ScanConfig, Duration, Timer};
use rtt_target::rprintln;
use embassy_executor::Spawner;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    rprintln!("PANIC: {:?}", info);
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_print!();
    // Setup the application (heap, peripherals, radio, etc.)
    let mut app = setup(spawner).await;

    loop {
        rprintln!("Scanning...");
        app.led.set_low(); // Turn LED ON while scanning

        let m = app.wifi.scan_with_config_async(ScanConfig::default()).await.unwrap();

        for ap in m {
            rprintln!("Found: {:?} | RSSI: {}", ap.ssid, ap.signal_strength);
        }

        app.led.set_high(); // Turn LED OFF
        Timer::after(Duration::from_secs(5)).await;
    }
}