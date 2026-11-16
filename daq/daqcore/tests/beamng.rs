use daqcore::{beamng, can_thread, connection::ConnectionSource};
use std::{
    net::UdpSocket,
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn source_settings_roundtrip_and_old_sources_still_load() {
    let source = ConnectionSource::BeamNg(4445);
    let json = serde_json::to_string(&source).unwrap();
    assert_eq!(
        serde_json::from_str::<ConnectionSource>(&json).unwrap(),
        source
    );
    assert!(source.display_name().contains("4445"));
    for json in [
        r#"{"Udp":5005}"#,
        r#""Loopback""#,
        r#"{"Simulated":true}"#,
        r#"{"Serial":["/dev/test","Kbps500"]}"#,
    ] {
        serde_json::from_str::<ConnectionSource>(json).unwrap();
    }
    assert!(serde_json::from_str::<ConnectionSource>(r#"{"BeamNg":65536}"#).is_err());
}

#[test]
fn udp_to_worker_to_plot_history() {
    // Reserve an available port long enough to find its address; the worker owns it next.
    let reservation = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    let (out, events) = mpsc::channel();
    let mut worker =
        can_thread::spawn_can_thread(can_thread::CanThreadConfig::default(), out).unwrap();
    worker
        .command(can_thread::CanThreadCommand::SelectDatabase {
            request_id: 1,
            selection: can_thread::DatabaseRequest::BeamNg,
        })
        .unwrap();
    drop(reservation);
    worker
        .command(can_thread::CanThreadCommand::Connect(Some(
            ConnectionSource::BeamNg(address.port()),
        )))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut loaded = false;
    loop {
        match events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            can_thread::CanThreadEvent::DatabaseSelected { result, .. } => {
                assert!(result.is_ok());
                loaded = true;
            }
            can_thread::CanThreadEvent::ConnectionSuccessful => break,
            can_thread::CanThreadEvent::ConnectionFailed(error) => panic!("{error}"),
            _ => {}
        }
    }
    assert!(loaded);
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    let packet = serde_json::json!({"protocol": beamng::PROTOCOL, "version": 1,
        "vehicle_id": 42, "sequence": 3, "sim_time": 2.0,
        "signals": {"speed_mps": 27.5, "wheel_FL_load_n": 3200, "wheel_FL_tire_avg_temp_c": 80}})
    .to_string();
    sender.send_to(b"malformed", address).unwrap();
    sender.send_to(packet.as_bytes(), address).unwrap();
    let mut router =
        daqcore::router::VirtualTimelineRouter::live(daqcore::time::Time::now(), 30.0).unwrap();
    let mut received = 0;
    while received < 6 {
        match events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            can_thread::CanThreadEvent::Frame(frame) => {
                let decoded = frame.decoded.as_ref().expect("built-in decoder active");
                if let Some(value) = decoded.signal("speed_mps") {
                    assert_eq!(value.physical(), 27.5);
                }
                if let Some(value) = decoded.signal("wheel_FL_load_n") {
                    assert_eq!(value.physical(), 3200.0);
                }
                if let Some(value) = decoded.signal("wheel_FL_tire_avg_temp_c") {
                    assert_eq!(value.physical(), 80.0);
                }
                router.ingest_frame(frame);
                received += 1;
            }
            can_thread::CanThreadEvent::ConnectionFailed(error) => panic!("{error}"),
            _ => {}
        }
    }
    assert_eq!(router.live_buffer().latest_map().len(), 6);
    worker.stop().unwrap();
}
