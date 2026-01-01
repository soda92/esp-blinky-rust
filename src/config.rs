use serde::{Serialize, Deserialize};
use heapless::String;
use postcard;
use sequential_storage::map::{MapConfig, MapStorage};
use sequential_storage::cache::NoCache;
use esp_storage::FlashStorage;
use esp_hal::peripherals::FLASH;
use embassy_embedded_hal::adapter::BlockingAsync;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub ssid: String<32>,
    pub password: String<64>,
    pub mqtt_host: String<64>,
    pub mqtt_port: u16,
    pub device_id: String<32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ssid: String::try_from("Guest").unwrap(),
            password: String::try_from("Guest1234").unwrap(),
            mqtt_host: String::try_from("192.168.1.100").unwrap(),
            mqtt_port: 1883,
            device_id: String::try_from("esp32-sensor").unwrap(),
        }
    }
}

const FLASH_ADDR_START: u32 = 0x9000; // NVS partition usually starts here on C3
const FLASH_SECTOR_SIZE: u32 = 4096;
const FLASH_PAGES: u32 = 4;
const FLASH_ADDR_END: u32 = FLASH_ADDR_START + FLASH_PAGES * FLASH_SECTOR_SIZE;

const CONFIG_KEY: u8 = 1;

pub struct ConfigStore<'a> {
    storage: MapStorage<u8, BlockingAsync<FlashStorage<'a>>, NoCache>,
}

impl<'a> ConfigStore<'a> {
    pub fn new(flash_peripheral: FLASH<'a>) -> Self {
        let flash = FlashStorage::new(flash_peripheral);
        let flash = BlockingAsync::new(flash);
        let config = MapConfig::new(FLASH_ADDR_START..FLASH_ADDR_END);
        let cache = NoCache::new();
        
        Self {
            storage: MapStorage::new(flash, config, cache),
        }
    }

    pub async fn save(&mut self, config: &AppConfig) -> Result<(), sequential_storage::Error<esp_storage::FlashStorageError>> {
        let mut buf = [0u8; 256]; // Work buffer for storage
        let mut ser_buf = [0u8; 256]; // Buffer for serialization
        
        // Serialize config to bytes
        let bytes = postcard::to_slice(config, &mut ser_buf).expect("Config serialization failed");
        let bytes: &[u8] = bytes;
        
        // store_item(buffer, key, value)
        self.storage.store_item(&mut buf, &CONFIG_KEY, &bytes).await
    }

    pub async fn load(&mut self) -> Result<AppConfig, sequential_storage::Error<esp_storage::FlashStorageError>> {
        let mut buf = [0u8; 256]; // Work buffer for storage and fetching
        
        // fetch_item(buffer, key)
        let res = self.storage.fetch_item::<&[u8]>(&mut buf, &CONFIG_KEY).await?;

        match res {
            Some(bytes) => {
                let config = postcard::from_bytes(bytes).unwrap_or(AppConfig::default());
                Ok(config)
            }
            None => Ok(AppConfig::default()),
        }
    }
}
