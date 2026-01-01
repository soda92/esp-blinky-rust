#![no_std]
#![no_main]

use esp_blinky_rust::{setup, Duration, Timer};
use esp_blinky_rust::config::ConfigStore;
use rtt_target::rprintln;
use embassy_executor::Spawner;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    rprintln!("PANIC: {:?}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_print!();
    let mut app = setup(spawner).await;

    // Initialize Config Store with Flash peripheral
    let mut config_store = ConfigStore::new(app.flash);
    let config = config_store.load().await.unwrap_or_default();

    rprintln!("Booting... SSID='{}'", config.ssid);

    loop {
        // Temperature sensor returns Temperature struct which has to_celsius()
        let temp = app.temp_sensor.get_temperature().to_celsius();
        
        rprintln!("Status: Running | Temp: {:.1} C | Config SSID: {}", temp, config.ssid);
        app.led.toggle();
        Timer::after(Duration::from_secs(2)).await;
    }
}