use crate::{can::driver, frame};
use daqcore::beamng::{MAX_PACKET_BYTES, parse_packet};
use std::{net::UdpSocket, time::Duration};

pub(crate) struct BeamNgDriver(UdpSocket);

impl BeamNgDriver {
    pub(crate) fn bind(port: u16) -> driver::DriverResult<Self> {
        let socket = UdpSocket::bind(("127.0.0.1", port))
            .map_err(|e| driver::DriverError::ConnectionFailed(e.to_string()))?;
        socket
            .set_read_timeout(Some(Duration::from_millis(10)))
            .map_err(|e| driver::DriverError::ConnectionFailed(e.to_string()))?;
        Ok(Self(socket))
    }
}

impl driver::Driver for BeamNgDriver {
    fn read_frames(&mut self) -> driver::DriverResult<Vec<frame::CanFrame>> {
        // An extra byte detects oversized datagrams even if the OS truncates them.
        let mut bytes = [0u8; MAX_PACKET_BYTES + 1];
        let len = self.0.recv(&mut bytes).map_err(|e| {
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) {
                driver::DriverError::Timeout
            } else {
                driver::DriverError::Read(e.to_string())
            }
        })?;
        // A bad UDP datagram must not tear down the listening socket/reconnect.
        Ok(parse_packet(&bytes[..len]).unwrap_or_default())
    }

    fn write_frame(&mut self, _: frame::CanFrameTx) -> driver::DriverResult<()> {
        Err(driver::DriverError::Unsupported(
            "BeamNG is a read-only telemetry source".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::can::driver::Driver;
    use daqcore::beamng::PROTOCOL;

    fn packet(signals: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"protocol": PROTOCOL, "version": 1,
            "vehicle_id": 42, "sequence": 7, "sim_time": 1.25, "signals": signals}))
        .unwrap()
    }

    #[test]
    fn udp_survives_bad_packets_and_cannot_transmit() {
        let mut driver = BeamNgDriver::bind(0).unwrap();
        let address = driver.0.local_addr().unwrap();
        assert!(address.ip().is_loopback());
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender.send_to(b"bad packet", address).unwrap();
        assert!(driver.read_frames().unwrap().is_empty());
        sender
            .send_to(&packet(serde_json::json!({"rpm": 6000})), address)
            .unwrap();
        assert_eq!(driver.read_frames().unwrap().len(), 4);
        assert!(matches!(
            driver.read_frames(),
            Err(driver::DriverError::Timeout)
        ));
        let tx = frame::CanFrameTx::data(Some(0), 1, false, vec![]).unwrap();
        assert!(matches!(
            driver.write_frame(tx),
            Err(driver::DriverError::Unsupported(_))
        ));
    }
}
