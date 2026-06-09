#!/usr/bin/env python3
"""
Fleet simulator.

Startup sequence:
  1. Create NUM_DELIVERIES delivery orders via POST /deliveries (concurrent).
  2. Backend auto-assigns free vehicles to deliveries.
  3. Poll GET /fleet/assignments every ASSIGNMENT_POLL_INTERVAL_S seconds and
     update each vehicle's steering target.

Vehicle movement:
  - If USE_OSRM=1 (default) and a local OSRM server is reachable, vehicles with
    an active assignment follow the real road network via OSRM waypoints.
  - If OSRM is disabled or the route fetch fails, vehicles drive in a straight
    line toward their destination (same behaviour as before).
  - If no assignment is known the vehicle does a bounded random walk.

Run scripts/run-osrm.sh before this script to start the local OSRM server.
"""

import asyncio
import math
import os
import random

import msgpack
import aiohttp

# ── Configuration ────────────────────────────────────────────────────────────

NUM_VEHICLES = int(os.environ.get("SIM_NUM_VEHICLES", 500))
NUM_DELIVERIES = 1500   # more than vehicles so there is always a queue
SERVER_URL = os.environ.get("BACKEND_URL", "http://localhost:3000")
OSRM_URL = os.environ.get("OSRM_URL", "http://localhost:5000")
USE_OSRM = os.environ.get("USE_OSRM", "1") != "0"

UPDATE_INTERVAL_S = 0.2           # telemetry cadence (200 ms)
ASSIGNMENT_POLL_INTERVAL_S = 2.0  # how often to refresh vehicle targets

# NYC bounding box
CENTER_LAT = 40.7128
CENTER_LNG = -74.0060
DELTA = 0.5        # ~5.5 km half-width
SPEED = 0.0005      # degrees per step ≈ 55 m per 200 ms

# Limit concurrent HTTP requests during the delivery-creation burst
CREATE_CONCURRENCY = 50


# ── Vehicle ───────────────────────────────────────────────────────────────────

class Vehicle:
    def __init__(self, vehicle_id: int, start_lat: float, start_lng: float):
        self.id = vehicle_id
        self.lat = start_lat
        self.lng = start_lng
        # Current steering target (straight-line fallback)
        self.target_lat: float | None = None
        self.target_lng: float | None = None
        # OSRM road waypoints; empty = straight-line or random walk
        self._waypoints: list[tuple[float, float]] = []
        self._wp_idx: int = 0

    # ── Target management ─────────────────────────────────────────────────────

    def set_target(self, lat: float, lng: float) -> bool:
        """Set a new destination. Returns True if the target actually changed."""
        changed = (self.target_lat, self.target_lng) != (lat, lng)
        if changed:
            self.target_lat = lat
            self.target_lng = lng
            self._waypoints = []
            self._wp_idx = 0
        return changed

    def clear_target(self) -> None:
        self.target_lat = None
        self.target_lng = None
        self._waypoints = []
        self._wp_idx = 0

    def set_route(self, waypoints: list[tuple[float, float]], dst_lat: float, dst_lng: float) -> None:
        """Apply OSRM waypoints only if the destination hasn't changed since the
        route was requested (guards against races during fast assignment cycling)."""
        if (self.target_lat, self.target_lng) == (dst_lat, dst_lng):
            self._waypoints = waypoints
            self._wp_idx = 0

    # ── Position update ───────────────────────────────────────────────────────

    def update(self) -> None:
        if self._wp_idx < len(self._waypoints):
            self._step_along_route()
        elif self.target_lat is not None:
            self._step_toward_target()
        # else: no assignment — hold current position

    def _step_along_route(self) -> None:
        target_lat, target_lng = self._waypoints[self._wp_idx]
        dlat = target_lat - self.lat
        dlng = target_lng - self.lng
        dist = math.hypot(dlat, dlng)
        if dist <= SPEED:
            self.lat, self.lng = target_lat, target_lng
            self._wp_idx += 1
        else:
            scale = SPEED / dist
            self.lat += dlat * scale
            self.lng += dlng * scale

    def _step_toward_target(self) -> None:
        dlat = self.target_lat - self.lat
        dlng = self.target_lng - self.lng
        dist_sq = dlat ** 2 + dlng ** 2
        if dist_sq <= SPEED ** 2:
            pass  # hold position — backend will assign the next delivery
        else:
            dist = math.sqrt(dist_sq)
            self.lat += (dlat / dist) * SPEED
            self.lng += (dlng / dist) * SPEED

    # ── Telemetry ─────────────────────────────────────────────────────────────

    async def send_telemetry(self, session: aiohttp.ClientSession) -> None:
        payload = {"vehicle_id": self.id, "lat": self.lat, "lng": self.lng}
        try:
            async with session.post(f"{SERVER_URL}/telemetry", json=payload):
                pass
        except Exception as exc:
            print(f"[vehicle {self.id}] telemetry error: {exc}")


# ── OSRM helper ───────────────────────────────────────────────────────────────

async def _osrm_route(
    session: aiohttp.ClientSession,
    src_lat: float, src_lng: float,
    dst_lat: float, dst_lng: float,
) -> list[tuple[float, float]]:
    """Request a driving route from OSRM. Returns [] on any failure."""
    url = f"{OSRM_URL}/route/v1/driving/{src_lng},{src_lat};{dst_lng},{dst_lat}"
    try:
        async with session.get(
            url,
            params={"overview": "full", "geometries": "geojson"},
            timeout=aiohttp.ClientTimeout(total=5),
        ) as resp:
            data = await resp.json()
        if data.get("code") == "Ok":
            # OSRM returns [longitude, latitude] — flip to (lat, lng)
            return [(lat, lng) for lng, lat in data["routes"][0]["geometry"]["coordinates"]]
    except Exception as exc:
        print(f"[OSRM] route error: {exc}")
    return []


async def _get_valid_road_point(session: aiohttp.ClientSession) -> tuple[float, float]:
    """
    Generates random coordinates and snaps them to the nearest valid road using OSRM.
    Rejects points that are too far from a road (e.g., deep in the ocean).
    """
    while True:
        lat = CENTER_LAT + (random.random() - 0.5) * DELTA
        lng = CENTER_LNG + (random.random() - 0.5) * DELTA

        if not USE_OSRM:
            return lat, lng

        url = f"{OSRM_URL}/nearest/v1/driving/{lng},{lat}"
        try:
            async with session.get(url, timeout=2) as resp:
                data = await resp.json()

            if data.get("code") == "Ok":
                wp = data["waypoints"][0]
                # If the original point is >300m from a road, it was probably in the water.
                # Reject it and try again to prevent coastal pile-ups.
                if wp["distance"] <= 300:
                    snapped_lng, snapped_lat = wp["location"]
                    return snapped_lat, snapped_lng
        except Exception:
            pass

        await asyncio.sleep(0.05) # Prevent CPU thrashing if OSRM is unreachable

async def _fetch_and_set_route(
    session: aiohttp.ClientSession,
    vehicle: Vehicle,
    dst_lat: float,
    dst_lng: float,
) -> None:
    waypoints = await _osrm_route(session, vehicle.lat, vehicle.lng, dst_lat, dst_lng)
    if waypoints:
        vehicle.set_route(waypoints, dst_lat, dst_lng)


# ── Background tasks ──────────────────────────────────────────────────────────

async def create_deliveries(session: aiohttp.ClientSession) -> None:
    """Bulk-create delivery orders at startup using a bounded concurrency pool."""
    print(f"Creating {NUM_DELIVERIES} delivery orders…")
    sem = asyncio.Semaphore(CREATE_CONCURRENCY)

    async def _create_one() -> None:

        road_lat, road_lng = await _get_valid_road_point(session)

        lat = road_lat + (random.random() - 0.5) * 0.00030
        lng = road_lng + (random.random() - 0.5) * 0.00030

        async with sem:
            try:
                async with session.post(
                    f"{SERVER_URL}/deliveries", json={"lat": lat, "lng": lng}
                ) as resp:
                    if resp.status not in (200, 201):
                        print(f"[create_deliveries] unexpected status {resp.status}")
            except Exception as exc:
                print(f"[create_deliveries] error: {exc}")

    await asyncio.gather(*[_create_one() for _ in range(NUM_DELIVERIES)])
    print("Delivery orders created.")


async def poll_assignments(
    vehicles: dict[int, Vehicle], session: aiohttp.ClientSession
) -> None:
    """Periodically fetch assignments and steer vehicles toward their destination.
    When USE_OSRM is enabled, fires off OSRM route requests for vehicles whose
    target changed since the last poll."""
    while True:
        await asyncio.sleep(ASSIGNMENT_POLL_INTERVAL_S)
        try:
            async with session.get(f"{SERVER_URL}/fleet/assignments") as resp:
                if resp.status != 200:
                    continue

                raw_data = await resp.read()
                assignments: list[dict] = msgpack.unpackb(raw_data)

            assigned_ids: set[int] = set()
            to_reroute: list[tuple[Vehicle, float, float]] = []

            for entry in assignments:
                vid = entry.get("vehicle_id")
                if vid not in vehicles:
                    continue
                v = vehicles[vid]
                dst_lat, dst_lng = entry["lat"], entry["lng"]
                assigned_ids.add(vid)
                if v.set_target(dst_lat, dst_lng):
                    # Target changed — schedule a fresh OSRM route
                    to_reroute.append((v, dst_lat, dst_lng))

            for v in vehicles.values():
                if v.id not in assigned_ids:
                    v.clear_target()

            if USE_OSRM and to_reroute:
                for v, dlat, dlng in to_reroute:
                    asyncio.create_task(_fetch_and_set_route(session, v, dlat, dlng))

        except Exception as exc:
            print(f"[poll_assignments] error: {exc}")


async def run_vehicle(vehicle: Vehicle, session: aiohttp.ClientSession) -> None:
    while True:
        vehicle.update()
        await vehicle.send_telemetry(session)
        await asyncio.sleep(UPDATE_INTERVAL_S)


# ── Entry point ───────────────────────────────────────────────────────────────

async def main() -> None:
    mode = f"OSRM road-following @ {OSRM_URL}" if USE_OSRM else "straight-line (OSRM disabled)"
    print(f"Starting simulation: {NUM_VEHICLES} vehicles, {NUM_DELIVERIES} deliveries — {mode}")

    async with aiohttp.ClientSession() as session:
        print("Generating valid spawn points for vehicles...")

        spawn_points = await asyncio.gather(
            *[_get_valid_road_point(session) for _ in range(NUM_VEHICLES)]
        )

        vehicles = {
            i + 1: Vehicle(i + 1, lat, lng)
            for i, (lat, lng) in enumerate(spawn_points)
        }

        await create_deliveries(session)

        await asyncio.gather(
            poll_assignments(vehicles, session),
            *[run_vehicle(v, session) for v in vehicles.values()],
        )


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\nSimulator stopped.")
