# ESP32 Temperature Monitor & Telemetry Stack

A complete IoT system that reads temperature from an ESP32-C3 device and visualizes it on a self-hosted dashboard.

## System Architecture

*   **Firmware**: Rust (`embassy-net`, `esp-hal`) running on ESP32-C3.
    *   Reads internal temperature sensor.
    *   Publishes data via MQTT.
*   **Infrastructure**:
    *   **Proxmox Host**: Handles network routing (NAT/Port Forwarding).
    *   **Linux Mint VM**: Hosts the backend services.
*   **Backend Stack**:
    *   **Mosquitto**: MQTT Broker.
    *   **TIG Stack**: Telegraf (Collector), InfluxDB (Storage), Grafana (Visualization).

## Documentation

*   **[Deployment Guide](docs/DEPLOY.md)**: Instructions for building firmware and deploying the server stack.
*   **[Network Routing](docs/ROUTING.md)**: Details on the Proxmox NAT and Port Forwarding setup.

## Quick Start (Firmware)

1.  Setup `config.json` (see Deployment Guide).
2.  Run `cargo run --release`.

## Project Structure

*   `src/bin/main.rs`: Main application logic (Wi-Fi, MQTT, Sensor loop).
*   `src/lib.rs`: Hardware initialization and `AppState`.
*   `docker-compose.yml`: Server-side service definition.
*   `telegraf.conf`: Configuration for data ingestion.
