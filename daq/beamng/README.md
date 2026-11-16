# BeamNG → DAQApp

Read-only vehicle telemetry from a Lua mod to DAQApp over **127.0.0.1:4445/UDP**.
Works with BeamNG running under Wine/BeamMac and DAQApp running natively on the
same Mac. No CAN hardware, BeamNG.tech license, OutGauge, or external DBC is needed.

## Build and install

From the repository root:

```sh
cargo build --manifest-path daq/Cargo.toml -p daqapp-rust --locked
python3 daq/beamng/package_mod.py
```

1. Launch BeamNG once to create its user folder.
2. In the BeamNG launcher, choose **Manage User Folder → Open in Explorer** to
   locate the active user folder. Copy `daq/beamng/dist/daqapp_telemetry.zip`
   into its `mods/` directory. Do **not** extract it or put it in Steam's game
   installation directory. Alternatively, add the ZIP using BeamMac's Mods page.
   For an explicit existing destination:
   `python3 daq/beamng/package_mod.py --mods-dir "/path/to/user-folder/mods"`.
3. Restart BeamNG if needed, and make sure the mod is enabled in Mod Manager.
4. Enter a map and spawn a vehicle. Open the console (`~`, depending on keyboard
   layout), select **GE Lua** (not Vehicle Lua), and run:

   ```lua
   extensions.load('daqApp')
   ```

   Enable it once per game session. It follows player vehicle switches, respawns,
   and level changes. It does not stream AI/traffic vehicles. To stop it:
   `extensions.unload('daqApp')`.
5. Start DAQApp from its own directory:

   ```sh
   cd daq/daqapp && cargo run --locked
   ```

6. Select **Source → BeamNG (localhost:4445)**. DAQApp loads the built-in BeamNG
   signal definitions automatically. **Listening** means the UDP socket is bound,
   not that the game is already sending.
7. Add a **CAN Viewer Table** to see received values. Add a **Scope**, select
   BeamNG messages/signals, and drive. For example plot `speed_mps`, `rpm`,
   `wheel_FL_load_n`, and `wheel_FL_slip_mps`.

The existing table, list, scope, timeline, and raw logger work with this source.
PER-specific battery/dynamics/G-G/GPS dashboards are not automatically remapped to
BeamNG channels. World X/Y coordinates are **not** latitude/longitude.

## Available signals

Definitions (including names, units, and descriptions) live in
`../daqcore/src/beamng.rs`, in the append-only `SIGNALS` table:

- Simulation time, packet sequence, and vehicle ID.
- Ground speed, wheel-based speed, RPM, gear, throttle, brake, clutch,
  steering input, parking brake, fuel level, coolant/oil temperature.
- Vehicle sensor acceleration (m/s²), world position/velocity, roll/pitch/yaw.
- For wheels explicitly named **FL, FR, RL, RR**: wheel speed, angular velocity,
  smoothed normal load, longitudinal/lateral slip velocity, propulsion torque,
  brake surface/core temperature, and average/core **tire temperatures in °C**
  (`wheel_FL_tire_avg_temp_c`, `wheel_FL_tire_core_temp_c`, and the FR/RL/RR
  equivalents). Tire values come from BeamNG's wheel APIs (Kelvin converted to
  Celsius); unavailable values and tireless wheels are omitted.

These wheel channels go beyond OutGauge. Vehicle-specific or unavailable fields
are **omitted**, not reported as zero. Custom wheel names/multi-axle vehicles need
an explicit mapping added to the Lua collector and schema. No suspension travel
or tire friction coefficient is inferred from unrelated values. Normal load is
not aerodynamic downforce, and slip velocity is not slip angle or slip ratio.
Acceleration retains BeamNG's sensor axes/signs and Z non-inertial/gravity term.

The default sampling target is **30 Hz, capped by graphics frame rate**. Collection
and JSON/network work run in `updateGFX`, not the 2 kHz physics loop. Pause stops
new samples; slow frames do not cause catch-up bursts. To change port/rate in GE Lua:

```lua
extensions.daqApp.configure(4446, 60)
```

Then change **BeamNG UDP Port** in DAQApp to match. Configuration in BeamNG lasts
for this game session; defaults return on reload. Only loopback is supported.
No game control commands are received, and DAQApp rejects transmit requests.

## Protocol and logs

One UTF-8 JSON object per datagram, maximum 16 KiB:

```json
{
  "protocol": "daqapp.beamng",
  "version": 1,
  "vehicle_id": 42,
  "sequence": 1,
  "sim_time": 0.0333333333,
  "signals": {"rpm": 4500, "speed_mps": 20, "wheel_FL_load_n": 3200}
}
```

Unknown signal names are ignored. Malformed, oversized, unsupported-version,
null-valued, or non-finite/Float32-overflowing packets are dropped without closing
the socket. Sparse samples are supported. UDP is best effort: packet delivery and
ordering are not guaranteed. Vehicle reset/switch can restart time and sequence.

The adapter creates synthetic 4-byte, little-endian Float32 CAN messages on
virtual bus **BeamNG (0)**, IDs `0x600 + index in SIGNALS`. These are an internal
compatibility representation, **not real vehicle CAN traffic**. All decoded
values, including counters and simulation time, have Float32 precision. Sequence
is exact through 2²⁴; at 30 Hz this is about six days of continuous operation.
Existing log files retain these synthetic frames. DAQApp's timeline uses **host
receipt timestamps**; `sim_time_s` is a separate vehicle clock and is not UTC.
Bus-load estimates are synthetic and must not be interpreted as real CAN utilization.

Export the exact SuperDBC definitions when parsing recorded BeamNG logs:

```sh
cargo run --manifest-path daq/Cargo.toml -p daqcore --example beamng_decode --locked -- --database > /tmp/beamng_superdbc.json
```

Choose that JSON with the log parser. Switching back to a physical CAN source
requires selecting the appropriate PER SuperDBC again.

## Tests (no running game needed)

From the repository root, with LuaJIT and Rust installed:

```sh
cargo test --manifest-path daq/Cargo.toml -p daqcore -p daqapp-rust --locked
luajit daq/beamng/tests/telemetry.lua > /tmp/beamng-packet.json
cargo run --manifest-path daq/Cargo.toml -p daqcore --example beamng_decode --locked < /tmp/beamng-packet.json
python3 daq/beamng/package_mod.py
```

The Lua tests mock game objects but execute the actual mod: rate limiting,
unavailable/NaN fields, pause/non-player filtering, reset, switches, respawns,
and socket cleanup. Their output passes through the actual Rust decoder. Rust
tests also cover UDP reception, bad packets, read-only transmission, saved source
compatibility, automatic definitions, and worker-to-plot-history ingestion.
A live in-game test is still needed to verify vehicle/version-specific fields.

## Troubleshooting

- **Listening but no values:** load the GE extension in a map, seat the player in
  the vehicle, unpause, check Mod Manager and matching ports. Check the BeamNG
  console/log for `daqApp` / `daqAppTelemetry` errors.
- **Address already in use:** close another DAQApp instance, or change both ports.
- **Missing wheel channels:** inspect the vehicle's wheel names; only explicit
  FL/FR/RL/RR corners are mapped. Do not guess axle/corner ordering.
- **Wine networking:** this adapter expects DAQApp and Wine on the same host,
  not a separate VM/container. A VM needs a separate, intentionally designed
  network setup; do not expose this unauthenticated listener to the LAN.
