#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
// 1. Add GPIO imports
use esp_hal::gpio::{ Level, Output, OutputConfig}; // <--- ADD THIS
use esp_radio::ble::controller::BleConnector;
use bt_hci::controller::ExternalController;
use trouble_host::prelude::*;

use rtt_target::rprintln;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    rprintln!("PANIC: {:?}", info);
    loop {}
}
use esp_radio::wifi::ScanConfig;

extern crate alloc;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.1.0

    rtt_target::rtt_init_print!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    rprintln!("Embassy initialized!");
    // 2. Setup GPIO 8 (Blue LED)
    // We do this before the radio stuff just to be safe, though order here usually doesn't matter
    let mut led = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());

    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let (mut wifi_controller, interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, esp_radio::wifi::Config::default())
            .expect("Failed to initialize Wi-Fi controller");

    // We need to take the station interface to ensure the controller knows we are in STA mode
    let _sta = interfaces.sta;
    
    // Set the mode to Station (Client)
    wifi_controller.set_config(&esp_radio::wifi::ModeConfig::Client(esp_radio::wifi::ClientConfig::default())).unwrap();
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    let transport = BleConnector::new(&radio_init, peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 1>::new(transport);
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let _stack = trouble_host::new(ble_controller, &mut resources);

    // TODO: Spawn some tasks
    let _ = spawner;

    // CHANGE 2: Start the WiFi controller!
    // You must turn it on before you can scan.
    rprintln!("Starting WiFi...");
    wifi_controller.start_async().await.unwrap();

    loop {
        rprintln!("Scanning...");
        led.set_low(); // Turn LED ON while scanning

        // CHANGE 3: The Scan Command
        // scan_n::<10> means "find up to 10 networks"
        let m = wifi_controller.scan_with_config_async(ScanConfig::default()).await.unwrap();

        for ap in m {
            rprintln!("Found: {:?} | RSSI: {}", ap.ssid, ap.signal_strength);
        }

        led.set_high(); // Turn LED OFF
        Timer::after(Duration::from_secs(5)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v~1.0/examples
}
