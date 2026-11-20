//! Read-only BeamNG telemetry over localhost UDP.
//!
//! Each scalar is represented as a synthetic Float32 CAN message on virtual bus 0.
//! This reuses decoding, history, scopes, and logging without sending anything to
//! a physical CAN bus. Frame timestamps are host receipt times; `sim_time_s`
//! preserves the vehicle's simulation clock (which resets with the vehicle).

use crate::{frame, superdbc};
use std::collections::BTreeMap;

pub const DEFAULT_PORT: u16 = 4445;
pub const PROTOCOL: &str = "daqapp.beamng";
pub const MAX_PACKET_BYTES: usize = 16_384;

// Stable order: append new signals; never reorder existing IDs in protocol v1.
// name, unit, description
pub const SIGNALS: &[(&str, &str, &str)] = &[
    (
        "sim_time_s",
        "s",
        "Simulation time since vehicle reset; not wall-clock time",
    ),
    (
        "sequence",
        "",
        "Packet counter; gaps indicate lost or skipped packets",
    ),
    ("vehicle_id", "", "BeamNG vehicle object ID"),
    ("speed_mps", "m/s", "Magnitude of world velocity"),
    (
        "wheel_speed_mps",
        "m/s",
        "Vehicle wheel-based speedometer speed",
    ),
    ("rpm", "rpm", "Engine speed"),
    ("gear", "", "Gear index; reverse is negative"),
    ("throttle", "", "Throttle input, 0 to 1"),
    ("brake", "", "Brake input, 0 to 1"),
    ("clutch", "", "Clutch input, 0 to 1"),
    ("steering", "", "Normalized steering input, -1 to 1"),
    ("parking_brake", "", "Parking brake input, 0 to 1"),
    (
        "fuel_fraction",
        "",
        "Fuel level, 0 to 1; absent when unavailable",
    ),
    ("coolant_temp_c", "degC", "Coolant temperature"),
    ("oil_temp_c", "degC", "Oil temperature"),
    (
        "accel_x_mps2",
        "m/s^2",
        "BeamNG sensors.gx, vehicle sensor X axis",
    ),
    (
        "accel_y_mps2",
        "m/s^2",
        "BeamNG sensors.gy, vehicle sensor Y axis",
    ),
    (
        "accel_z_mps2",
        "m/s^2",
        "BeamNG sensors.gz, includes non-inertial/gravity component",
    ),
    ("position_x_m", "m", "World position X; not GPS longitude"),
    ("position_y_m", "m", "World position Y; not GPS latitude"),
    ("position_z_m", "m", "World position Z"),
    ("velocity_x_mps", "m/s", "World velocity X"),
    ("velocity_y_mps", "m/s", "World velocity Y"),
    ("velocity_z_mps", "m/s", "World velocity Z"),
    ("roll_rad", "rad", "Vehicle roll"),
    ("pitch_rad", "rad", "Vehicle pitch"),
    ("yaw_rad", "rad", "Vehicle yaw"),
    (
        "wheel_FL_speed_mps",
        "m/s",
        "FL signed wheel circumferential speed",
    ),
    (
        "wheel_FL_angular_velocity",
        "rad/s",
        "FL wheel angular velocity",
    ),
    (
        "wheel_FL_load_n",
        "N",
        "FL smoothed wheel normal load; not aerodynamic downforce",
    ),
    (
        "wheel_FL_slip_mps",
        "m/s",
        "FL native longitudinal slip velocity",
    ),
    (
        "wheel_FL_side_slip_mps",
        "m/s",
        "FL native lateral slip velocity; not slip angle",
    ),
    ("wheel_FL_drive_torque_nm", "Nm", "FL propulsion torque"),
    (
        "wheel_FL_brake_temp_c",
        "degC",
        "FL brake surface temperature",
    ),
    (
        "wheel_FL_brake_core_temp_c",
        "degC",
        "FL brake core temperature",
    ),
    (
        "wheel_FR_speed_mps",
        "m/s",
        "FR signed wheel circumferential speed",
    ),
    (
        "wheel_FR_angular_velocity",
        "rad/s",
        "FR wheel angular velocity",
    ),
    (
        "wheel_FR_load_n",
        "N",
        "FR smoothed wheel normal load; not aerodynamic downforce",
    ),
    (
        "wheel_FR_slip_mps",
        "m/s",
        "FR native longitudinal slip velocity",
    ),
    (
        "wheel_FR_side_slip_mps",
        "m/s",
        "FR native lateral slip velocity; not slip angle",
    ),
    ("wheel_FR_drive_torque_nm", "Nm", "FR propulsion torque"),
    (
        "wheel_FR_brake_temp_c",
        "degC",
        "FR brake surface temperature",
    ),
    (
        "wheel_FR_brake_core_temp_c",
        "degC",
        "FR brake core temperature",
    ),
    (
        "wheel_RL_speed_mps",
        "m/s",
        "RL signed wheel circumferential speed",
    ),
    (
        "wheel_RL_angular_velocity",
        "rad/s",
        "RL wheel angular velocity",
    ),
    (
        "wheel_RL_load_n",
        "N",
        "RL smoothed wheel normal load; not aerodynamic downforce",
    ),
    (
        "wheel_RL_slip_mps",
        "m/s",
        "RL native longitudinal slip velocity",
    ),
    (
        "wheel_RL_side_slip_mps",
        "m/s",
        "RL native lateral slip velocity; not slip angle",
    ),
    ("wheel_RL_drive_torque_nm", "Nm", "RL propulsion torque"),
    (
        "wheel_RL_brake_temp_c",
        "degC",
        "RL brake surface temperature",
    ),
    (
        "wheel_RL_brake_core_temp_c",
        "degC",
        "RL brake core temperature",
    ),
    (
        "wheel_RR_speed_mps",
        "m/s",
        "RR signed wheel circumferential speed",
    ),
    (
        "wheel_RR_angular_velocity",
        "rad/s",
        "RR wheel angular velocity",
    ),
    (
        "wheel_RR_load_n",
        "N",
        "RR smoothed wheel normal load; not aerodynamic downforce",
    ),
    (
        "wheel_RR_slip_mps",
        "m/s",
        "RR native longitudinal slip velocity",
    ),
    (
        "wheel_RR_side_slip_mps",
        "m/s",
        "RR native lateral slip velocity; not slip angle",
    ),
    ("wheel_RR_drive_torque_nm", "Nm", "RR propulsion torque"),
    (
        "wheel_RR_brake_temp_c",
        "degC",
        "RR brake surface temperature",
    ),
    (
        "wheel_RR_brake_core_temp_c",
        "degC",
        "RR brake core temperature",
    ),
];

/// Exportable definitions for decoding both live data and recorded synthetic frames.
pub fn database_json() -> String {
    let messages: Vec<_> = SIGNALS
        .iter()
        .enumerate()
        .map(|(index, (name, unit, description))| {
            serde_json::json!({
                "id": 0x600 + index, "message_name": format!("BeamNG_{name}"),
                "length_bytes": 4, "is_extended_id": false, "transmitter": "BeamNG",
                "receivers": [], "nominal_period_ms": 34, "priority": 0,
                "description": description,
                "signals": [{
                    "signal_name": name, "description": description, "data_type": "float",
                    "raw_type": "float32", "start_bit": 0, "bit_length": 32,
                    "byte_order": "little_endian", "scale": 1.0, "offset": 0.0,
                    "unit": unit, "display_format": "3f", "choices": null, "limits": null
                }]
            })
        })
        .collect();
    serde_json::json!({
        "versions": {"schema_version": "1.1", "hash": "beamng-v1"},
        // Virtual definitions have no firmware build/content hash.
        "content_hash": "0".repeat(64),
        "buses": {"BeamNG": {"bus_id": 0, "baud_rate": 500000,
            "nodes": [{"name": "BeamNG", "is_external": true}], "messages": messages}}
    })
    .to_string()
}

pub fn database() -> Result<superdbc::database::Database, superdbc::error::ParseError> {
    superdbc::database::Database::from_json(&database_json())
}

#[derive(serde::Deserialize)]
struct Packet {
    protocol: String,
    version: u8,
    vehicle_id: u32,
    sequence: u32,
    sim_time: f64,
    signals: BTreeMap<String, f64>,
}

/// Validate the complete packet before producing frames. Unknown signals are
/// ignored for forward compatibility; missing signals are never fabricated as zero.
pub fn parse_packet(bytes: &[u8]) -> Result<Vec<frame::CanFrame>, String> {
    if bytes.len() > MAX_PACKET_BYTES {
        return Err("BeamNG packet exceeds 16 KiB".into());
    }
    let mut packet: Packet = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if packet.protocol != PROTOCOL || packet.version != 1 {
        return Err("Unsupported BeamNG telemetry protocol/version".into());
    }
    packet.signals.insert("sim_time_s".into(), packet.sim_time);
    packet
        .signals
        .insert("sequence".into(), packet.sequence as f64);
    packet
        .signals
        .insert("vehicle_id".into(), packet.vehicle_id as f64);
    let mut frames = Vec::with_capacity(SIGNALS.len());
    for (index, (name, _, _)) in SIGNALS.iter().enumerate() {
        let Some(&value) = packet.signals.get(*name) else {
            continue;
        };
        let value32 = value as f32;
        if !value.is_finite() || !value32.is_finite() {
            return Err(format!("Non-finite or overflowing BeamNG signal {name}"));
        }
        frames.push(frame::CanFrame::data(
            Some(0),
            0x600 + index as u32,
            value32.to_le_bytes().to_vec(),
        )?);
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(signals: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"protocol": PROTOCOL, "version": 1,
            "vehicle_id": 42, "sequence": 7, "sim_time": 1.25, "signals": signals}))
        .unwrap()
    }

    #[test]
    fn definitions_and_packet_decode_agree() {
        let database = database().unwrap();
        let bus = database.bus_by_id(0).unwrap();
        let signals: serde_json::Map<_, _> = SIGNALS
            .iter()
            .map(|(name, _, _)| ((*name).to_owned(), serde_json::json!(-12.5)))
            .collect();
        let frames = parse_packet(&packet(signals.into())).unwrap();
        assert_eq!(frames.len(), SIGNALS.len());
        for frame in frames {
            assert_eq!(frame.bus_id, Some(0));
            assert_eq!(frame.data.len(), 4);
            bus.decode(frame.identity, &frame.data).unwrap();
        }
        let speed = parse_packet(&packet(serde_json::json!({"speed_mps": 23.75}))).unwrap();
        let speed = speed.iter().find(|f| f.identity == 0x603).unwrap();
        assert_eq!(
            f32::from_le_bytes(speed.data.clone().try_into().unwrap()),
            23.75
        );
    }

    #[test]
    fn sparse_packets_omit_unavailable_values_and_ignore_unknown_fields() {
        let frames = parse_packet(&packet(
            serde_json::json!({"rpm": 3000, "future_signal": 123}),
        ))
        .unwrap();
        assert_eq!(frames.len(), 4); // three metadata values plus RPM
        assert!(frames.iter().any(|f| f.identity == 0x605));
        assert!(!frames.iter().any(|f| f.identity == 0x603));
    }

    #[test]
    fn rejects_bad_versions_malformed_null_and_overflow() {
        assert!(parse_packet(b"not json").is_err());
        assert!(parse_packet(&vec![b' '; MAX_PACKET_BYTES + 1]).is_err());
        assert!(parse_packet(&packet(serde_json::json!({"rpm": null}))).is_err());
        assert!(parse_packet(&packet(serde_json::json!({"rpm": 1e100}))).is_err());
        let mut data: serde_json::Value =
            serde_json::from_slice(&packet(serde_json::json!({}))).unwrap();
        data["version"] = 2.into();
        assert!(parse_packet(&serde_json::to_vec(&data).unwrap()).is_err());
        data["version"] = 1.into();
        data["protocol"] = "other".into();
        assert!(parse_packet(&serde_json::to_vec(&data).unwrap()).is_err());
    }
}
