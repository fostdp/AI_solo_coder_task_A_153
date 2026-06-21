import json
import time
import random
import argparse
import paho.mqtt.client as mqtt


SPINDLE_COUNT = 8
BASE_RPM = 1500
BASE_VIBRATION = 0.15
BASE_TEMPERATURE = 35.0
BASE_TWIST = 800
MQTT_TOPIC = "spindle/sensor_data"


def generate_spindle_data(spindle_id, tick):
    rpm = BASE_RPM + 200 * math.sin(tick * 0.05 + spindle_id) + random.gauss(0, 30)
    rpm = max(500, min(4000, rpm))

    vibration = BASE_VIBRATION + 0.1 * math.sin(tick * 0.1 + spindle_id * 0.7) + random.gauss(0, 0.02)
    if rpm > 2500:
        vibration += (rpm - 2500) * 0.0003
    vibration = max(0.01, min(2.0, vibration))

    temperature = BASE_TEMPERATURE + (rpm / 1000.0) * 5 + random.gauss(0, 1.0)
    temperature = max(20.0, min(100.0, temperature))

    twist = BASE_TWIST + 50 * math.sin(tick * 0.03 + spindle_id * 1.3) + random.gauss(0, 20)
    twist = max(500, min(1200, twist))

    return {
        "spindle_id": f"SPD-{spindle_id:03d}",
        "rpm": round(rpm, 2),
        "vibration_amplitude": round(vibration, 4),
        "temperature": round(temperature, 2),
        "twist_per_meter": round(twist, 1),
    }


def on_connect(client, userdata, flags, rc):
    if rc == 0:
        print(f"Connected to MQTT broker at {userdata['host']}:{userdata['port']}")
    else:
        print(f"Connection failed with code {rc}")


def main():
    parser = argparse.ArgumentParser(description="Ancient Water Spindle Sensor Simulator")
    parser.add_argument("--host", default="localhost", help="MQTT broker host")
    parser.add_argument("--port", type=int, default=1883, help="MQTT broker port")
    parser.add_argument("--interval", type=int, default=60, help="Publish interval in seconds")
    parser.add_argument("--spindles", type=int, default=SPINDLE_COUNT, help="Number of spindles to simulate")
    args = parser.parse_args()

    client = mqtt.Client(userdata={"host": args.host, "port": args.port})
    client.on_connect = on_connect

    client.connect(args.host, args.port, 60)
    client.loop_start()

    tick = 0
    try:
        while True:
            for sid in range(1, args.spindles + 1):
                data = generate_spindle_data(sid, tick)
                payload = json.dumps(data)
                result = client.publish(MQTT_TOPIC, payload, qos=1)
                if result.rc == mqtt.MQTT_ERR_SUCCESS:
                    print(f"[{time.strftime('%H:%M:%S')}] {data['spindle_id']}: RPM={data['rpm']:.0f} VIB={data['vibration_amplitude']:.3f}mm TEMP={data['temperature']:.1f}C TWIST={data['twist_per_meter']:.0f}/m")
                else:
                    print(f"Publish failed for {data['spindle_id']}")
            tick += 1
            time.sleep(args.interval)
    except KeyboardInterrupt:
        print("\nSimulator stopped.")
    finally:
        client.loop_stop()
        client.disconnect()


import math

if __name__ == "__main__":
    main()
