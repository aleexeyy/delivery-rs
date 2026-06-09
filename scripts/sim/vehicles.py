#!/usr/bin/env python3
import asyncio
import random
import time

import aiohttp

# Configuration
NUM_VEHICLES = 500
SERVER_URL = "http://localhost:3000/telemetry"
UPDATE_INTERVAL_MS = 200  # milliseconds

# Bounding box (NYC area)
CENTER_LAT = 40.7128
CENTER_LNG = -74.0060
DELTA = 0.05  # ~5.5 km
SPEED = 0.0005  # degrees per 200ms (~50 m per step)


class Vehicle:
    def __init__(self, vehicle_id: int):
        self.id = vehicle_id
        self.lat = CENTER_LAT + (random.random() - 0.5) * DELTA
        self.lng = CENTER_LNG + (random.random() - 0.5) * DELTA
        self.vx = (random.random() - 0.5) * SPEED
        self.vy = (random.random() - 0.5) * SPEED

    def update(self):
        self.lat += self.vx
        self.lng += self.vy
        # bounce off edges
        if abs(self.lat - CENTER_LAT) > DELTA:
            self.vx *= -1
        if abs(self.lng - CENTER_LNG) > DELTA:
            self.vy *= -1

    async def send_telemetry(self, session: aiohttp.ClientSession):
        data = {"vehicle_id": self.id, "lat": self.lat, "lng": self.lng}
        try:
            async with session.post(SERVER_URL, json=data) as resp:
                pass  # ignore response for speed
        except Exception as e:
            print(f"Vehicle {self.id} error: {e}")


async def run_vehicle(vehicle: Vehicle, session: aiohttp.ClientSession):
    while True:
        vehicle.update()
        await vehicle.send_telemetry(session)
        await asyncio.sleep(UPDATE_INTERVAL_MS / 1000.0)


async def main():
    print(f"Starting {NUM_VEHICLES} vehicles...")
    vehicles = [Vehicle(i) for i in range(1, NUM_VEHICLES + 1)]
    async with aiohttp.ClientSession() as session:
        tasks = [asyncio.create_task(run_vehicle(v, session)) for v in vehicles]
        await asyncio.gather(*tasks)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\nSimulator stopped.")
