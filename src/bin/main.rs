#![no_std]
#![no_main]

use esp_blinky_rust::{setup, ScanConfig, Duration, Timer};
use esp_blinky_rust::config::{ConfigStore, AppConfig};
use rtt_target::rprintln;
use embassy_executor::Spawner;
use embassy_time::with_timeout;
use embedded_io_async::{Read, Write};
use heapless::String;

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
    let mut config = config_store.load().await.unwrap_or_default();

    rprintln!("Booting... SSID='{}'", config.ssid);
    rprintln!("Press 's' to enter setup...");

    let mut buf = [0u8; 1];
    // Wait for input with timeout
    let result = with_timeout(Duration::from_secs(5), app.serial.read(&mut buf)).await;

    if let Ok(Ok(n)) = result {
        if n > 0 && buf[0] == b's' {
             rprintln!("Entering Setup Mode via Serial...");
             let _ = run_setup(&mut app.serial, &mut config_store, &mut config).await;
        }
    }

    loop {
        // Temperature sensor returns Temperature struct which has to_celsius()
        let temp = app.temp_sensor.get_temperature().to_celsius();
        
        rprintln!("Status: Running | Temp: {:.1} C | Config SSID: {}", temp, config.ssid);
        app.led.toggle();
        Timer::after(Duration::from_secs(2)).await;
    }
}

async fn run_setup<S>(serial: &mut S, store: &mut ConfigStore<'_>, config: &mut AppConfig) -> Result<(), S::Error>
where S: Read + Write + Unpin {
    serial.write_all(b"\r\n--- Setup Mode ---\r\n").await?;
    
    serial.write_all(b"Enter SSID: ").await?;
    let ssid = read_line(serial).await?;
    if !ssid.is_empty() {
        config.ssid = String::try_from(ssid.as_str()).unwrap_or(config.ssid.clone());
    }

    serial.write_all(b"Enter Password: ").await?;
    let pass = read_line(serial).await?;
    if !pass.is_empty() {
        config.password = String::try_from(pass.as_str()).unwrap_or(config.password.clone());
    }
    
    serial.write_all(b"Enter MQTT Host: ").await?;
    let host = read_line(serial).await?;
    if !host.is_empty() {
        config.mqtt_host = String::try_from(host.as_str()).unwrap_or(config.mqtt_host.clone());
    }

    match store.save(config).await {
        Ok(_) => serial.write_all(b"Configuration Saved!\r\n").await?,
        Err(_) => serial.write_all(b"Error Saving Config!\r\n").await?,
    }
    
    serial.write_all(b"Continuing boot...\r\n").await?;
    Ok(())
}

async fn read_line<S>(serial: &mut S) -> Result<String<64>, S::Error>
where S: Read + Unpin {
    let mut buf = [0u8; 64];
    let mut pos = 0;
    loop {
        let mut b = [0u8; 1];
        serial.read(&mut b).await?;
        let c = b[0];
        if c == b'\n' || c == b'\r' {
            break;
        }
        if pos < buf.len() {
            buf[pos] = c;
            pos += 1;
        }
    }
    let s = core::str::from_utf8(&buf[..pos]).unwrap_or("");
    Ok(String::try_from(s).unwrap_or(String::new()))
}