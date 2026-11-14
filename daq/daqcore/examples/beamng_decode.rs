//! Decode a mod-produced packet from stdin, or export its SuperDBC definitions.
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--database") {
        println!("{}", daqcore::beamng::database_json());
        return Ok(());
    }
    let mut bytes = Vec::new();
    io::stdin().take(16_385).read_to_end(&mut bytes)?;
    let database = daqcore::beamng::database()?;
    let bus = database.bus_by_id(0).unwrap();
    let frames = daqcore::beamng::parse_packet(&bytes).map_err(io::Error::other)?;
    for frame in frames {
        for (name, value) in bus.decode(frame.identity, &frame.data)?.iter() {
            println!("{name}: {}", value.physical());
        }
    }
    Ok(())
}
