import sys
import time
import argparse
import random

try:
    import paho.mqtt.client as mqtt
except ImportError:
    print("Error: 'paho-mqtt' library not found.")
    print("Please install it using: pip install paho-mqtt")
    sys.exit(1)

def on_connect(client, userdata, flags, rc):
    if rc == 0:
        print(f"✅ Connected to MQTT Broker at {userdata['host']}!")
        # Subscribe to a test topic
        client.subscribe("test/topic")
        # Publish a message
        msg = f"Hello from Python! Random: {random.randint(1, 100)}"
        client.publish("test/topic", msg)
        print(f"📤 Published message: '{msg}'")
    else:
        print(f"❌ Failed to connect, return code {rc}")

def on_message(client, userdata, msg):
    print(f"wb Received message on {msg.topic}: {msg.payload.decode()}")
    client.disconnect()

def main():
    parser = argparse.ArgumentParser(description='Simple MQTT Connectivity Tester')
    parser.add_argument('host', help='IP address or hostname of the MQTT broker')
    parser.add_argument('--port', type=int, default=1883, help='Port number (default: 1883)')
    
    args = parser.parse_args()

    client = mqtt.Client(userdata={'host': args.host})
    client.on_connect = on_connect
    client.on_message = on_message

    print(f"Attempting to connect to {args.host}:{args.port}...")
    
    try:
        client.connect(args.host, args.port, 60)
        client.loop_forever()
    except Exception as e:
        print(f"❌ Connection failed: {e}")
        print("Tip: Check if the firewall is allowing port 1883 and if 'listener 1883' is set in mosquitto.conf")

if __name__ == "__main__":
    main()
