# Deployment Guide

This guide covers building/flashing the ESP32 firmware and deploying the telemetry stack (TIG) on the server.

## 1. ESP32 Firmware

### Prerequisites
*   Rust toolchain (stable)
*   `espup` (for ESP32 toolchain support)
*   `probe-rs` (for flashing)

### Configuration
Create a `config.json` file in the project root. This file is ignored by git for security.

```json
{
    "ssid": "YOUR_WIFI_SSID",
    "password": "YOUR_WIFI_PASSWORD",
    "mqtt_host": "192.168.0.107", 
    "mqtt_port": 1883,
    "device_id": "esp32_temp_sensor"
}
```
*Note: `mqtt_host` should be the IP of your Proxmox host (Gateway), which forwards to the Mint VM.*

### Build & Flash
```bash
# Build release binary
cargo build --release

# Flash to device and monitor logs
cargo run --release
```

## 2. Server Stack (Linux Mint)

We use **Docker** to run the TIG stack (Telegraf, InfluxDB, Grafana) for collecting and visualizing data.

### Prerequisites
*   Docker & Docker Compose (v1 or v2 plugin) installed on the Mint VM (`10.10.10.3`).
*   Mosquitto MQTT Broker installed and running natively on the Mint VM (`sudo apt install mosquitto`).

### Deployment Steps

1.  **Transfer Configs**:
    Copy `docker-compose.yml` and `telegraf.conf` to the server:
    ```bash
    scp docker-compose.yml telegraf.conf user@10.10.10.3:~/iot-stack/
    ```

2.  **Start Services**:
    SSH into the server and launch the stack:
    ```bash
    ssh user@10.10.10.3
    cd ~/iot-stack
    docker-compose up -d
    ```

3.  **Fix Permissions** (If Grafana fails to start):
    Grafana runs as user 472. If volumes are owned by root, it crashes.
    ```bash
    sudo chown -R 472:472 grafana_data
    docker-compose restart grafana
    ```

### Accessing Dashboards

*   **Grafana**: [http://10.10.10.3:3000](http://10.10.10.3:3000)
    *   **Default Login**: `admin` / `admin`
    *   **Data Source**: Configured for InfluxDB (`http://influxdb:8086`, DB: `sensors`).
*   **InfluxDB**: [http://10.10.10.3:8086](http://10.10.10.3:8086)

### Data Flow
1.  **Mosquitto**: Receives `sensors/temp` from ESP32.
2.  **Telegraf**: Subscribes to `sensors/temp` (defined in `telegraf.conf`) and pushes to InfluxDB.
3.  **InfluxDB**: Stores time-series data.
4.  **Grafana**: Queries InfluxDB for visualization.
