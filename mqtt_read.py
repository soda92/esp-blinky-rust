import sys
import argparse

try:
    import paho.mqtt.client as mqtt
except ImportError:
    print("Error: 'paho-mqtt' library not found.")
    sys.exit(1)

# Global counter for received messages
msg_count = 0
MAX_MESSAGES = 3

def on_connect(client, userdata, flags, rc):
    if rc == 0:
        print(f"✅ Connected to MQTT Broker at {userdata['host']}!")
        topic = "sensors/temp"
        client.subscribe(topic)
        print(f"📡 Subscribed to '{topic}'. Waiting for {MAX_MESSAGES} messages...")
    else:
        print(f"❌ Failed to connect, return code {rc}")
        sys.exit(1)

def on_message(client, userdata, msg):
    global msg_count
    try:
        payload = msg.payload.decode()
        print(f"[{msg_count + 1}/{MAX_MESSAGES}] 🌡️  Temperature: {payload} °C")
        msg_count += 1
        if msg_count >= MAX_MESSAGES:
            print("✅ Received enough messages. Exiting.")
            client.disconnect()
    except Exception as e:
        print(f"Error decoding message: {e}")

def main():
    parser = argparse.ArgumentParser(description='Read MQTT Temperature Data')
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

if __name__ == "__main__":
    main()
