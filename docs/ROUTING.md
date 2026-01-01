# Network Routing Architecture

This document describes the network topology and routing configuration used to enable communication between the ESP32 device (Local LAN) and the backend services running on a virtualized Linux Mint server (Proxmox NAT).

## Overview

*   **ESP32 Device**: Connects to the physical Local LAN (Wi-Fi).
*   **Proxmox Host**: Acts as the gateway/bridge.
    *   **Physical IP**: `192.168.0.107` (Access point for ESP32).
    *   **Bridge (`vmbr0`)**: `10.10.10.1`.
*   **Linux Mint VM**: Hosts the MQTT broker and TIG stack.
    *   **Internal IP**: `10.10.10.3`.
    *   **Network Mode**: NAT (Isolated behind Proxmox).

## Port Forwarding (Proxmox)

Since the Mint VM is on an isolated NAT network (`10.10.10.0/24`), external devices on the physical LAN cannot reach it directly. We use **DNAT (Destination NAT)** on the Proxmox host to forward specific ports.

### Active Forwarding Rules

Traffic hitting the Proxmox Host (`192.168.0.107`) on these ports is forwarded to the Mint VM (`10.10.10.3`):

| Service | Port | Protocol | Description |
| :--- | :--- | :--- | :--- |
| **MQTT** | `1883` | TCP | Telemetry from ESP32 to Mosquitto Broker. |

### IPTables Configuration

The following rules have been added to `/etc/network/interfaces` on the Proxmox host to ensure persistence across reboots:

```bash
# /etc/network/interfaces on Proxmox Host

auto vmbr0
iface vmbr0 inet static
    address 10.10.10.1/24
    # ... other config ...

    # MQTT Port Forwarding for Mint VM (10.10.10.3)
    post-up iptables -t nat -A PREROUTING -p tcp --dport 1883 -j DNAT --to-destination 10.10.10.3:1883
    post-up iptables -t nat -A POSTROUTING -p tcp -d 10.10.10.3 --dport 1883 -j MASQUERADE
    
    post-down iptables -t nat -D PREROUTING -p tcp --dport 1883 -j DNAT --to-destination 10.10.10.3:1883
    post-down iptables -t nat -D POSTROUTING -p tcp -d 10.10.10.3 --dport 1883 -j MASQUERADE
```

## Troubleshooting

If the ESP32 cannot connect to MQTT:

1.  **Check Proxmox IP**: Ensure the ESP32 is configured to connect to `192.168.0.107` (Proxmox), NOT `10.10.10.3`.
2.  **Verify Forwarding**: Run `iptables -t nat -L -n -v` on the Proxmox host to see if packets are matching the DNAT rule.
3.  **Check VM**: Ensure the Mint VM firewall allows port 1883 and Mosquitto is running (`systemctl status mosquitto`).
