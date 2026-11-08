//! ALSA sequencer client: one port, subscribed both ways to a VirMIDI port that Bitwig opens
//! as a rawmidi device (Bitwig only lists rawmidi devices, not sequencer-only ports).
//! Same approach as jimpo/mikro-mk3-driver.

use std::error::Error;
use std::ffi::CString;

use alsa::poll::Descriptors;
use alsa::seq::{Addr, ClientIter, EventType, MidiEvent, PortCap, PortIter, PortSubscribe, PortType, Seq};
use libc::pollfd;

pub struct Midi {
    seq: Seq,
    port: i32,
    encoder: MidiEvent,
    decoder: MidiEvent,
}

/// Is `name` the sequencer port of a VirMIDI device's first subdevice (`VirMIDI N-0`)?
fn is_virmidi(name: &str) -> bool {
    name.strip_prefix("VirMIDI ")
        .and_then(|n| n.strip_suffix("-0"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn matches(want: &str, addr: Addr, name: &str) -> bool {
    if want == name {
        return true;
    }
    let mut it = want.split(':');
    match (it.next().map(str::parse::<i32>), it.next().map(str::parse::<i32>), it.next()) {
        (Some(Ok(c)), Some(Ok(p)), None) => addr.client == c && addr.port == p,
        _ => false,
    }
}

impl Midi {
    /// `target`: port to connect to, by name or `client:port`; default: the one `VirMIDI N-0`.
    pub fn open(target: Option<&str>) -> Result<Midi, Box<dyn Error>> {
        let seq = Seq::open(None, None, true).map_err(|e| format!("can't open the ALSA sequencer: {e}"))?;
        let name = CString::new("Maschine MK3").unwrap();
        seq.set_client_name(&name)?;
        let port = seq.create_simple_port(
            &name,
            PortCap::READ | PortCap::SUBS_READ | PortCap::WRITE | PortCap::SUBS_WRITE,
            PortType::MIDI_GENERIC | PortType::APPLICATION,
        )?;

        let mut found = vec![];
        for client in ClientIter::new(&seq) {
            for p in PortIter::new(&seq, client.get_client()) {
                let pname = p.get_name().unwrap_or_default().to_string();
                let hit = match target {
                    Some(t) => matches(t, p.addr(), &pname),
                    None => is_virmidi(&pname),
                };
                if hit {
                    found.push((p.addr(), pname));
                }
            }
        }
        let peer = match (&found[..], target) {
            ([(addr, _)], _) => *addr,
            ([], None) => {
                return Err("no VirMIDI port found; load snd-virmidi \
                            (sudo modprobe snd-virmidi midi_devs=1 index=10)"
                    .into())
            }
            ([], Some(t)) => return Err(format!("no sequencer port named {t}").into()),
            (many, _) => {
                let names: Vec<_> = many.iter().map(|(a, n)| format!("{}:{} {n}", a.client, a.port)).collect();
                return Err(format!(
                    "several matching ports, pass one with --port NAME or client:port: {}",
                    names.join(", ")
                )
                .into());
            }
        };

        let me = Addr { client: seq.client_id()?, port };
        for (from, to) in [(me, peer), (peer, me)] {
            let sub = PortSubscribe::empty()?;
            sub.set_sender(from);
            sub.set_dest(to);
            seq.subscribe_port(&sub)?;
        }

        let encoder = MidiEvent::new(256)?;
        let decoder = MidiEvent::new(256)?;
        decoder.enable_running_status(false);
        eprintln!("maschine-mk3: connected to sequencer port {}:{}", peer.client, peer.port);
        Ok(Midi { seq, port, encoder, decoder })
    }

    /// Sends one complete MIDI message (or sysex) to Bitwig.
    pub fn send(&mut self, msg: &[u8]) -> Result<(), Box<dyn Error>> {
        self.encoder.reset_encode();
        let (_, ev) = self.encoder.encode(msg)?;
        let mut ev = ev.ok_or("incomplete MIDI message")?;
        ev.set_source(self.port);
        ev.set_subs();
        ev.set_direct();
        self.seq.event_output_direct(&mut ev)?;
        Ok(())
    }

    /// Calls `f` with every pending incoming message (sysex may arrive in fragments).
    pub fn drain(&mut self, mut f: impl FnMut(&[u8])) -> Result<(), Box<dyn Error>> {
        let mut input = self.seq.input();
        loop {
            let mut ev = match input.event_input() {
                Ok(ev) => ev,
                Err(e) if e.errno() == libc::EAGAIN => return Ok(()),
                Err(e) if e.errno() == libc::ENOSPC => continue, // overrun
                Err(e) => return Err(e.into()),
            };
            if ev.get_type() == EventType::Sysex {
                if let Some(data) = ev.get_ext() {
                    f(data);
                }
                continue;
            }
            let mut buf = [0; 256];
            if let Ok(n) = self.decoder.decode(&mut buf, &mut ev) {
                if n > 0 {
                    f(&buf[..n]);
                }
            }
        }
    }

    pub fn fds(&self) -> Result<Vec<pollfd>, Box<dyn Error>> {
        Ok((&self.seq, Some(alsa::Direction::Capture)).get()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virmidi_names() {
        assert!(is_virmidi("VirMIDI 10-0"));
        assert!(!is_virmidi("VirMIDI 10-1"));
        assert!(!is_virmidi("Midi Through Port-0"));
    }

    #[test]
    fn target() {
        let a = Addr { client: 24, port: 0 };
        assert!(matches("24:0", a, "VirMIDI 6-0"));
        assert!(matches("VirMIDI 6-0", a, "VirMIDI 6-0"));
        assert!(!matches("24:1", a, "VirMIDI 6-0"));
    }
}
