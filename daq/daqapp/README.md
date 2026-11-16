# PER DaqApp

DaqApp is PER's complete trackside data acquisition and analysis desktop application/tooling.

## What it does

- Connects to supported CAN sources and decodes traffic with a selected SuperDBC artifact and CAN bus selection
- Receives live BeamNG vehicle telemetry through the [DAQApp Lua mod](../beamng/README.md), with built-in signal definitions
- Displays live and historical signals in configurable tables, lists, multi-unit scopes, state lanes, XY scatter plots, and GPS maps
- Built-in Subteam Workspace Presets (`daq/daqapp/default_presets/*.daqview.json`) for Vehicle Dynamics, Powertrain, Thermals, Aerodynamics, Sensors & Controls, and Trackside Live Debug
- Portable `.daqview.json` workspace layouts and compressed `daqview1:...` Slack/Discord share strings with optional time-range and custom formula restoration
- 3-click Dataset Export Modal (`Parquet`, `CSV`, and MATLAB v5 `.mat`) powered by `daq/config/export_presets.json`
- Logs all received CAN traffic and imports SD card logs into canonical Arrow Parquet partitions with multi-resolution pyramid tiles
- Full support for CAN-message sending
- Hardware-in-the-loop configuration, test running, and results viewing
- Full PER Bootloader workflow

## Screenshots

<table>
  <tr>
    <td><img src="./showcase/table.png" width="500"></td>
    <td><img src="./showcase/battery_charging_workflow.png" width="500"></td>
  </tr>
  <tr>
    <td><img src="./showcase/hil.png" width="500"></td>
    <td><img src="./showcase/scope.png" width="500"></td>
  </tr>
</table>

## Using the app

1. Use sidebar to select a source from the connection controls (SLCAN, CANnector USB, UDP, BeamNG, or dev modes). For BeamNG, follow the [mod setup guide](../beamng/README.md); its signal definitions load automatically, so skip step 2.
2. Use the sidebar to select a `superdbc_*.json` artifact and its CAN bus or multi-bus preset. The file picker opens in `monorepo/superdbc`, which is an output from the firmware build process
3. Use the sidebar (or ctrl-P) to launch widgets
4. Use **Bootloader** to validate and upload a package. See the
   [bootloader update guide](../../docs/daqapp/bootloader_updates.md).

Sidebar settings (source, SuperDBC, etc) are saved to a local settings file to persist between sessions.

## Scope graphs

- Click the plot to move the shared playhead. Left drag selects a time range and zooms the shared timeline to it. Escape cancels the selection.
- Right drag pans the shared window. Scroll zooms time, Shift scroll zooms Y, and double click resets the local Y zoom.
- The legend icon toggles the bottom left overlay. Eye icons temporarily hide signals without deleting them or changing their colors.
- Click a legend entry, then **Appearance**, **Assignment**, or **Axis** to open its settings. Configure color, opacity, width, lines, points, stepped rendering, plot assignment, and independent Auto or Fixed axes.
- The stacked plots icon splits signals into aligned lanes. Drag a legend color swatch into another plot to move that signal. The scope menu also contains plot count and layout controls.
- The measurement icon enables A and B cursors. Click twice to pin them, then drag either cursor line. The plots show the time difference and legends show signal differences. Missing or offscreen measurements are not interpolated across gaps.
- Appearance, axes, and lane assignments are stored in workspace presets. Temporary visibility and measurement cursors are session state.

## Getting started

Install Rust/Cargo and the platform prerequisites described in the repository
[setup guide](../../docs/setup.md). Linux users also likely need `libudev-dev` and
`pkg-config` to discover serial devices.

### To Build

From the repository root:

```bash
cargo build --manifest-path daq/daqapp/Cargo.toml --locked
```

From the `daq/daqapp/` directory, you can also build with Cargo directly:

```bash
cargo build --locked
```

### To Run

Starting from the repository root, run the app with:

```bash
cd daq/daqapp && cargo run
```

Running is supported only with `daq/daqapp/` as the working directory so the
app can find its themes, formatter configuration, and HIL presets. Settings and
logs are also relative to this directory.

## Development

Useful commands from `daq/daqapp/`:

```bash
cargo fmt
cargo build --locked
cargo run
```

The app's source is organized by responsibility:

- `src/ui/` - application views and panels.
- `../daqcore/src/can_thread/` - CAN worker and operational commands/events.
- `../daqcore/src/superdbc/` - bus-aware definitions, decoding, and encoding.
- `../daqcore/src/log_parse/` - recorded-log parsing and correlation.
- `../daqcore/src/hil/` and `hil_config/` - HIL support and presets
- `themes/` - user-selectable visual themes.

### Thread overview

![daqapp_overview](../../docs/daqapp/daqapp_overview.drawio.png)
![daqapp_threads](../../docs/daqapp/daqapp_threads.drawio.png)

### Testing

For easy testing, you can use the loopback or simulated CAN sources. The loopback source is a virtual CAN bus that echoes messages sent to it, while the simulated source generates random messages for testing purposes.
