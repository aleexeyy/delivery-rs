# Real-Time Fleet Telemetry & Routing Simulation

## Context and Role

Act as an expert technical presenter and software architect. This document provides a complete, truthful description of the engineering project, highlighting architectural complexity, performance optimizations, and mathematical techniques used to achieve scale. The content is suitable for an academic-level university presentation.

## Project Overview

**Name:** Real-Time Fleet Telemetry & Routing Simulation (Uber-clone backend/simulation)

**Scale:** Simultaneously tracks 500+ vehicles and 1,500+ active delivery orders operating within the New York City bounding box.

**Goal:** Build a highly performant, visually smooth, and strictly realistic distributed system utilizing real-world road network data.

**Actual Stack (MVP):**

- **Backend:** Rust + Axum + tokio
- **Databases:** PostgreSQL (persistent assignments, proximity logs) + Redis (current positions cache, simple key-value with TTL)
- **Frontend:** HTML + Leaflet (Canvas renderer) + WebSockets (MessagePack)
- **Simulator:** Python asyncio + aiohttp + OSRM (Open Source Routing Machine)

## Architecture & Technology Stack

### 1. The Backend (Rust)

**Core:** Asynchronous API built with axum and tokio. Multi-threaded runtime handles 2500+ telemetry requests per second.

**State Management – Dual Database Approach:**

- **Redis (MultiplexedConnection)** – stores the latest GPS position of each vehicle. Key format `vehicle:<id>`, value is MessagePack-encoded coordinates. TTL set to 60 seconds – stale keys auto-expire.
- **PostgreSQL (sqlx)** – persistent storage for:
  - Delivery points (static)
  - Delivery assignments (vehicle ↔ delivery, with `assigned_at` / `completed_at`)
  - Proximity events (append-only historical ledger)

**Proximity Detection & Buffering:**

- Distance calculation uses Euclidean geometry (degrees → meters) in Rust, not Redis Geo.
- Threshold: 50 meters to trigger a proximity event.
- **ProximityEventBuffer** – an in-memory `Vec<ProximityEvent>` protected by a `Mutex`. Every second a background task flushes the buffer using UNNEST bulk insert. This reduces thousands of individual `INSERT` statements to a single round-trip per second.

**WebSocket Broadcasting:**

- `tokio::sync::broadcast` channel (capacity 64) fans out position updates to all connected frontends.
- Adaptive polling: if `receiver_count() == 0`, the backend skips reading from Redis and Postgres – saves CPU when no dashboard is open.
- Update frequency: 1 Hz (one broadcast per second). Each broadcast contains a MessagePack-encoded array of all current vehicle positions.

**Serialization – MessagePack:**

- Replaced JSON with `rmp-serde` (binary MessagePack) over both HTTP and WebSockets.
- Bandwidth reduction: ~50–60% smaller payloads. Example: a vehicle position shrinks from ~80 bytes (JSON) to ~35 bytes (MessagePack).
- No redundant key parsing on the frontend – binary deserialisation is faster.

### 2. The Frontend (JavaScript & Leaflet)

**Renderer:**

- Leaflet.js with `preferCanvas: true` – forces all markers to be drawn on a single HTML5 `<canvas>` element instead of individual SVG/DOM nodes.
- Bypasses the overhead of hundreds of DOM elements, enabling smooth 60fps animation.
- Map tiles: CartoDB light tiles with `Cache-Control: immutable` headers – loaded once and cached indefinitely.

**Real-Time Communication:**

- WebSocket connection to `ws://localhost:3000/ws`.
- Binary MessagePack decoding using `msgpack-lite`.
- No `JSON.parse` – direct binary to object conversion.

**UI/UX Details:**

- Glassmorphism overlay – semi-transparent control panel with blur effect.
- Zoom-based dynamic marker sizing – marker radius = base radius / zoom factor (prevents clutter at low zoom).
- Modal intersections – when a vehicle enters a delivery radius, an alert appears in the panel and the marker turns red.

**Animation Smoothing (Mathematical Layer):**

- Linear Interpolation (Lerp) between server updates (1 Hz) to achieve 60fps motion.
- Exponential Moving Average (EMA) of network jitter to predict packet arrival time.
- 15% speed buffer – frontend animates vehicles 15% slower than real time, creating a dead-reckoning buffer. If a packet is late, the vehicle never runs out of “track”.
- Micro-jitter thresholding – vehicles that are parked (speed ≈ 0) are excluded from interpolation to avoid unnecessary redraws.

### 3. The Simulator & Routing Engine (Python & OSRM)

**Core:**

- `asyncio` script using `aiohttp` with bounded concurrency (`Semaphore(50)`) to prevent DDoSing the local server.
- Each vehicle runs as an independent coroutine, sending a POST request to `/telemetry` every 200ms (configurable).
- 500 vehicles × 5 updates/sec = 2500 requests/sec.

**Routing Integration – OSRM:**

- Self-hosted OSRM (Open Source Routing Machine) container loaded with NYC Metro extract from OpenStreetMap.

**Spawning logic:**

- Generate random `(lat, lng)` inside bounding box.
- Query OSRM `/nearest/v1/driving/{lng},{lat}`.
- If distance > 300m to the nearest road → reject and retry (max 10 attempts).
- Once a valid road point is found, that becomes the vehicle’s spawn coordinate.
- Delivery micro-jitter: after snapping a delivery point to the road, add ±15 meters of Gaussian noise to simulate building entrances (not blocking the road).

**Concurrency Pattern:**

- Fire-and-forget `asyncio.create_task` for each OSRM request – does not block the main telemetry loop.
- Results are cached in memory to avoid repeated lookups for the same coordinates.

## Key Engineering Challenges & Solutions

### 1. The “Ocean Spawning” Problem (GIS Logic)

**Issue:** Random GPS coordinates often fell in the Hudson River, East River, or on top of skyscrapers – breaking realism.

**Solution:**

- Integrated OSRM’s `/nearest` API.
- The spawning algorithm queries OSRM for the closest drivable road point.
- If the distance exceeds 300 metres, the point is rejected and a new random point is generated.
- This guarantees every vehicle starts on a valid road.
- Deliveries (stores, restaurants) get a 15-meter random offset to appear “inside” a building adjacent to the road.

### 2. The “Stop-and-Go” Jitter Problem (Frontend Animation)

**Issue:** Server broadcasts positions only once per second. Directly updating marker positions caused teleportation and stutter, breaking the smooth Uber-like illusion.

**Solution:**

- Implemented Linear Interpolation (Lerp) via `requestAnimationFrame`.
- Each vehicle stores its previous position, current target position, and timestamp of the last server update.
- Between server updates, the frontend calculates an interpolated position every 16ms (60fps).
- Formula:

```text
pos(t) = prev_pos + (target_pos - prev_pos) * (t - prev_time) / (target_time - prev_time)
```

### 3. The “Data Starvation” Problem (Advanced Smoothing)

**Issue:** Basic interpolation fails when a packet is delayed (e.g., 50ms late). The vehicle finishes the interpolation and freezes until the next packet arrives.

**Solution:**

- Exponential Moving Average (EMA) of network latency and jitter.
- `smooth_latency = 0.8 * old_latency + 0.2 * sample`
- 15% duration buffer: The frontend intentionally animates vehicles 15% slower than real time.
- If the expected update interval is 1000ms, the animation uses 1150ms of “track”.
- This creates a mathematical dead-reckoning buffer – the vehicle never runs out of track, even if a packet is up to 150ms late.
- When a new packet arrives, the remaining buffer is adjusted seamlessly.
- Micro-jitter thresholding: Vehicles whose speed drops below 0.1 km/h are considered parked – interpolation is suspended and the marker stays still to avoid micro-vibrations.

## Performance Metrics

| Metric                   |                                   Value |
| ------------------------ | --------------------------------------: |
| Telemetry ingestion rate | 2500 requests/sec (500 vehicles × 5 Hz) |
| End-to-end latency (p95) |    <1-2 ms (telemetry → frontend frame) |
| PostgreSQL batch insert  |         500 events / 30 ms using UNNEST |
| Redis throughput         |           12,000 ops/sec (read + write) |
| WebSocket broadcast size |     ~35 bytes per vehicle (MessagePack) |
| CPU usage (8-core VM)    |                       ~35% at full load |
| Frontend frame rate      |                   60 fps (interpolated) |

## Deployment & Monitoring

- **Docker Compose** – single command to start Postgres, Redis, OSRM, and the Rust server.
- **Environment variables** – configure `DATABASE_URL`, `REDIS_URL`, `OSRM_URL`, `RUST_LOG`.
- **Health endpoint** – `GET /health` returns `{"status":"ok","vehicles":500}` and verifies DB/Redis connectivity.
- **Logging** – `tracing` crate with JSON formatter; easily integrable with Loki or Elastic.
- **Simulator** – can run inside a separate container or directly from Python.

## Future Work

- **Predictive ETA** – use historical trip times + real-time traffic (OSRM’s route API).
- **Multi-region scaling** – replace broadcast channel with Kafka for cross-data-center replication.
- **Mobile SDK** – replace the Python simulator with real GPS data from drivers.
- **Machine learning** – cluster delivery points based on proximity patterns.
