#![no_std]
#![deny(clippy::large_stack_frames)]

use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::tsens::{TemperatureSensor, Config as TsensConfig};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use esp_hal::Async;
use esp_hal::peripherals::FLASH;
use esp_radio::ble::controller::BleConnector;
use bt_hci::controller::ExternalController;
use trouble_host::prelude::*;
use rtt_target::rprintln;
use esp_radio::wifi::{WifiController, Config, ModeConfig, ClientConfig};
use alloc::boxed::Box;

extern crate alloc;

pub mod config;

// Re-exports for main.rs
pub use esp_radio::wifi::ScanConfig;
pub use embassy_time::{Duration, Timer};
pub use embassy_executor::Spawner;
pub use rtt_target::rtt_init_print;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

// Define the type for the BLE Stack to simplify return types
pub type BleStack<'a> = Stack<'a, ExternalController<BleConnector<'a>, 1>, DefaultPacketPool>;

pub struct AppState {
    pub led: Output<'static>,
    pub wifi: WifiController<'static>,
    pub ble_stack: BleStack<'static>,
    pub temp_sensor: TemperatureSensor<'static>,
    pub serial: UsbSerialJtag<'static, Async>,
    pub flash: FLASH<'static>,
}

pub async fn setup(_spawner: Spawner) -> AppState {
    // 1. Initialize Heap
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    esp_alloc::heap_allocator!(size: 64 * 1024);

    // 2. Initialize Peripherals
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // 3. Initialize RTOS & Interrupts
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    rprintln!("Embassy initialized!");

    // 4. Initialize LED
    let led = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());

    // 5. Initialize Sensor and Serial
    let temp_sensor = TemperatureSensor::new(peripherals.TSENS, TsensConfig::default()).expect("Failed to init TSENS");
    
    let serial = UsbSerialJtag::new(peripherals.USB_DEVICE).into_async();

    // 6. Initialize Radio (WiFi & BLE)
    // We leak the radio_init to get a 'static reference, allowing us to return controllers
    // that reference it.
    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let radio_init: &'static _ = Box::leak(Box::new(radio_init));
    
    // WiFi Setup
    let (mut wifi_controller, interfaces) =
        esp_radio::wifi::new(radio_init, peripherals.WIFI, Config::default())
            .expect("Failed to initialize Wi-Fi controller");
    
    // Ensure STA mode
    let _sta = interfaces.sta;
    wifi_controller.set_config(&ModeConfig::Client(ClientConfig::default())).unwrap();
    
    // Start WiFi
    rprintln!("Starting WiFi...");
    wifi_controller.start_async().await.unwrap();

    // BLE Setup
    let transport = BleConnector::new(radio_init, peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 1>::new(transport);
    
    // Use StaticCell for resources to ensure they live forever
    static RESOURCES: static_cell::StaticCell<HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>> = static_cell::StaticCell::new();
    let resources = RESOURCES.init(HostResources::new());
    
    let ble_stack = trouble_host::new(ble_controller, resources);

    rprintln!("Setup complete.");

    AppState {
        led,
        wifi: wifi_controller,
        ble_stack,
        temp_sensor,
        serial,
        flash: peripherals.FLASH,
    }
}
