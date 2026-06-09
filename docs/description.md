CONTEXT AND ROLE

Act as an expert technical presenter and software architect. Generate a compelling, academic-level university presentation (Slide Titles, Speaker Notes, and Visual Suggestions) based on the following engineering project. The presentation must highlight the architectural complexity, performance optimizations, and mathematical techniques used to achieve scale.

PROJECT OVERVIEW

Name: Real-Time Fleet Telemetry & Routing Simulation (Uber-clone backend/simulation).

Scale: Simultaneously tracks 500+ vehicles and 1,500+ active delivery orders operating within the New York City bounding box.

Goal: Build a highly performant, visually smooth, and strictly realistic distributed system utilizing real-world road network data.

ARCHITECTURE & TECHNOLOGY STACK

1. The Backend (Rust)

Core: Asynchronous API built with axum and tokio.

State Management: Dual-database approach.

Redis (MultiplexedConnection) for high-frequency, volatile telemetry data (GPS pings).

PostgreSQL (sqlx) for persistent state (assignments, orders). Strict schema with TEXT enums and bulk UNNEST queries for high-throughput background buffered inserts (ProximityEventBuffer).

Broadcasting: Bounded tokio::sync::broadcast channels push state to WebSockets at strict 1000ms intervals. Pauses DB/Redis polling automatically if receiver_count() == 0 to save CPU.

Serialization: Replaced JSON with Binary MessagePack (rmp-serde) over WebSockets and HTTP. Reduces bandwidth consumption by ~50-60% and eliminates redundant key parsing.

2. The Frontend (JavaScript & Leaflet)

Renderer: Uses Leaflet.js mapped to CartoDB tiles. Enforces a unified <canvas> renderer (bypassing DOM/SVG overhead) to smoothly draw hundreds of elements simultaneously.

Security & Caching: Strict CORS/ORB compliance. Uses Cache-Control: immutable and crossOrigin: 'anonymous' for instantaneous tile/asset loading.

UI/UX: Glassmorphism overlay, responsive zoom-based dynamic marker sizing, and modal intersections using cached interpolation data.

3. The Simulator & Routing Engine (Python & OSRM)

Core: asyncio script utilizing aiohttp with bounded concurrency (Semaphores) to prevent DDoSing the local server.

Routing: Integrates a self-hosted OSRM (Open Source Routing Machine) container loaded with NYC map data.

Concurrency: Utilizes fire-and-forget asyncio tasks to request real-world driving vectors from OSRM without blocking the main polling loop.

KEY ENGINEERING CHALLENGES & SOLUTIONS (Highlight these in the presentation)

The "Ocean Spawning" Problem (GIS Logic):

Issue: Random GPS coordinate generation spawned cars in the Hudson River and on top of skyscrapers.

Solution: Integrated OSRM's /nearest API. System generates random points, queries OSRM, and rejects points >300m from a road (ocean). Vehicles spawn exactly on road coordinates. Deliveries spawn with a 15-meter mathematical "micro-jitter" to simulate being inside a building adjacent to the road.

The "Stop-and-Go" Jitter Problem (Frontend Animation):

Issue: 1000ms server pings caused vehicles to teleport or freeze due to network latency/jitter, breaking the Uber-like illusion.

Solution: Implemented 60fps continuous Linear Interpolation (Lerp) via requestAnimationFrame.

The "Data Starvation" Problem (Advanced Smoothing):

Issue: Basic interpolation fails if a packet is 50ms late (the car runs out of "track" and freezes).

Solution: Implemented Exponential Moving Average (EMA) network smoothing with a 15% duration buffer. The frontend intentionally animates vehicles 15% slower than real-time. This acts as a mathematical "dead-reckoning" buffer, ensuring seamless handoffs between coordinates so vehicles never stop moving. Also implemented micro-jitter thresholding to allow parked cars to sleep.

EXPECTED OUTPUT STRUCTURE

Generate a 10-12 slide presentation. For each slide, provide:

Slide Title

Visual Layout: (What charts, code snippets, or diagrams should be on the screen)

Bullet Points: (Highly compressed technical text for the slide)

Speaker Notes: (A conversational but deeply technical script explaining the why and how behind the bullet points, referencing the challenges and solutions above).
