//! Maschine MK3 userspace driver for Bitwig Studio on Linux.
//!
//! Reproduces NI's MIDI mode (NIHardwareAgent + Controller Editor) so that Bitwig's built-in
//! "Native Instruments → Maschine Mk3" extension works as on Windows: pads, buttons, knobs,
//! LEDs, pad colours and the text on the two displays.

mod hidraw;
mod protocol;
mod screen;
mod seq;
mod translate;

use std::error::Error;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use hidraw::Hidraw;
use libc::{pollfd, POLLERR, POLLHUP, POLLIN, POLLNVAL};
use protocol::*;
use screen::Screens;
use seq::Midi;
use translate::{Config, PadMode, Translator};

const USAGE: &str = "\
usage: maschine-mk3 [run] [OPTIONS]   driver (default)
       maschine-mk3 dump              print raw HID reports and decoded controls
       maschine-mk3 led-test [VALUE]  light every LED one by one
       maschine-mk3 screen-test       draw a test pattern on both displays

options (run):
  --port NAME|C:P        sequencer port to connect to (default: the one 'VirMIDI N-0')
  --no-screens           don't use the displays
  --knob-divider N       knob sensitivity, raw units per MIDI step (default 13; smaller = faster;
                         4x finer while SHIFT is held)
  --pad-mode events|threshold   how pad hits are detected (default events)
  --pad-threshold N      threshold mode: raw pressure 0..4095 (default 128)
  --pad-vel-max N        raw pad value giving velocity 127 (default 2048; smaller = louder)
  --pad-curve X          velocity curve: 1 = linear, <1 = soft hits louder (default 0.6)
  --pad-touch-trigger N  light touches reaching this pressure play a note (default 70, 0 = off)
  --knob-hysteresis N    extra movement needed to reverse a knob (default 6, against jitter)
  --led-dim N            LED value for an idle button backlight (default 124 = 0x7c)
  --led-on N             LED value for a lit button (default 126 = 0x7e)";

struct Opts {
    port: Option<String>,
    screens: bool,
    cfg: Config,
}

fn parse_num(s: Option<String>) -> Result<i64, String> {
    let s = s.ok_or("missing value")?;
    let r = match s.strip_prefix("0x") {
        Some(h) => i64::from_str_radix(h, 16),
        None => s.parse(),
    };
    r.map_err(|_| format!("bad number: {s}"))
}

fn parse_opts(mut args: impl Iterator<Item = String>) -> Result<Opts, String> {
    let mut o = Opts { port: None, screens: true, cfg: Config::default() };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => o.port = Some(args.next().ok_or("--port needs a value")?),
            "--no-screens" => o.screens = false,
            "--knob-divider" => o.cfg.knob_divider = parse_num(args.next())?.clamp(1, 1000) as i32,
            "--pad-threshold" => o.cfg.pad_threshold = parse_num(args.next())?.clamp(0, 4095) as u16,
            "--pad-vel-max" => o.cfg.pad_vel_max = parse_num(args.next())?.clamp(1, 4095) as u16,
            "--pad-touch-trigger" => o.cfg.pad_touch_trigger = parse_num(args.next())?.clamp(0, 4095) as u16,
            "--knob-hysteresis" => o.cfg.knob_hysteresis = parse_num(args.next())?.clamp(0, 500) as i32,
            "--pad-curve" => {
                let v = args.next().ok_or("--pad-curve needs a value")?;
                o.cfg.pad_curve = v.parse::<f32>().map_err(|_| format!("bad number: {v}"))?.clamp(0.1, 3.0);
            }
            "--led-dim" => o.cfg.led_dim = parse_num(args.next())?.clamp(0, 255) as u8,
            "--led-on" => o.cfg.led_on = parse_num(args.next())?.clamp(0, 255) as u8,
            "--pad-mode" => {
                o.cfg.pad_mode = match args.next().as_deref() {
                    Some("events") => PadMode::Events,
                    Some("threshold") => PadMode::Threshold,
                    _ => return Err("--pad-mode: events or threshold".into()),
                }
            }
            _ => return Err(format!("unknown option {a}")),
        }
    }
    Ok(o)
}

/// Blocks SIGINT and SIGTERM and returns a signalfd that becomes readable when one arrives.
fn signalfd() -> io::Result<OwnedFd> {
    // SAFETY: plain libc calls on a locally owned sigset_t.
    unsafe {
        let mut set = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        if libc::sigprocmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = libc::signalfd(-1, &set, libc::SFD_NONBLOCK | libc::SFD_CLOEXEC);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(OwnedFd::from_raw_fd(fd))
    }
}

type LedState = ([u8; BUTTON_LEDS], [u8; PAD_LED_BYTES]);

enum LedCmd {
    Show(LedState),
    /// All dark, then stop.
    Quit,
}

/// Writes the LEDs from their own thread. A hidraw write is synchronous (the kernel waits for
/// the USB transfer), so doing it in the main loop delayed the next pad hits by a few ms each
/// time Bitwig answered with new colours. Only the latest state is written, and only the
/// report that changed.
fn spawn_leds(mut hid: Hidraw) -> (mpsc::Sender<LedCmd>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<LedCmd>();
    let handle = thread::spawn(move || {
        let (mut shown_btn, mut shown_pad) = (None, None);
        while let Ok(mut cmd) = rx.recv() {
            while let Ok(newer) = rx.try_recv() {
                if !matches!(cmd, LedCmd::Quit) {
                    cmd = newer;
                }
            }
            let ((btn, pad), quit) = match cmd {
                LedCmd::Show(s) => (s, false),
                LedCmd::Quit => (([0; BUTTON_LEDS], [0; PAD_LED_BYTES]), true),
            };
            let mut result = Ok(());
            if shown_btn != Some(btn) {
                result = hid.write_report(REPORT_LEDS_BUTTONS, &btn, 63);
                shown_btn = Some(btn);
            }
            if result.is_ok() && shown_pad != Some(pad) {
                result = hid.write_report(REPORT_LEDS_PADS, &pad, 63);
                shown_pad = Some(pad);
            }
            if let Err(e) = result {
                eprintln!("maschine-mk3: LED write failed ({e})");
                return; // the main loop notices the device is gone on its own
            }
            if quit {
                return;
            }
        }
    });
    (tx, handle)
}

fn open_screens(enabled: bool) -> Option<Screens> {
    if !enabled {
        return None;
    }
    match Screens::open() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("maschine-mk3: displays unavailable ({e}); continuing without them");
            None
        }
    }
}

fn run(o: Opts) -> Result<(), Box<dyn Error>> {
    let signals = signalfd()?;
    let mut hid = Hidraw::open()?;
    eprintln!("maschine-mk3: using {}", hid.path.display());
    let mut midi = Midi::open(o.port.as_deref())?;
    let screens = open_screens(o.screens).map(screen::spawn);
    let mut t = Translator::new(o.cfg);
    match hid.get_input(REPORT_BUTTONS, 42) {
        Some(r) => t.init(&r),
        None => eprintln!("maschine-mk3: no initial state (GET_REPORT), using the first report"),
    }
    let (leds, leds_thread) = spawn_leds(hid.try_clone()?);
    let mut sent_leds = t.leds();
    let _ = leds.send(LedCmd::Show(sent_leds));
    // Bitwig answers by sending all its LED and display state.
    midi.send(&SYSEX_RETURN_FROM_HOST)?;

    let seq_fds = midi.fds()?;
    let mut fds = vec![
        pollfd { fd: hid.fd(), events: POLLIN, revents: 0 },
        pollfd { fd: signals.as_raw_fd(), events: POLLIN, revents: 0 },
    ];
    fds.extend(&seq_fds);
    let mut buf = [0u8; 512];

    loop {
        // Hand the display text to the display thread when it changed (it never blocks here).
        if t.display_dirty {
            t.display_dirty = false;
            if let Some((tx, _)) = &screens {
                let _ = tx.send(screen::Cmd::Show(Box::new(t.display), t.midi_mode));
            }
        }

        // SAFETY: fds is a valid, exclusively borrowed array of pollfd.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e.into());
        }
        if fds[0].revents & (POLLERR | POLLHUP | POLLNVAL) != 0 {
            return Err("device gone".into());
        }
        if fds[1].revents != 0 {
            // SIGINT / SIGTERM: leave the device dark.
            let _ = leds.send(LedCmd::Quit);
            let _ = leds_thread.join();
            if let Some((tx, thread)) = screens {
                let _ = tx.send(screen::Cmd::Quit);
                let _ = thread.join();
            }
            return Ok(());
        }
        if fds[0].revents & POLLIN != 0 {
            while let Some(report) = hid.read(&mut buf)? {
                for msg in t.input(report) {
                    midi.send(&msg)?;
                }
            }
        }
        if fds[2..].iter().any(|p| p.revents != 0) {
            midi.drain(|msg| t.midi(msg))?;
        }
        // Hand the LEDs to the LED thread only when they changed (never blocks here).
        let now = t.leds();
        if now != sent_leds {
            sent_leds = now;
            let _ = leds.send(LedCmd::Show(now));
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

/// Prints every HID report, and what the driver understands from it.
fn dump() -> Result<(), Box<dyn Error>> {
    let mut hid = Hidraw::open()?;
    println!("device: {}", hid.path.display());
    let mut ins: Vec<_> = hid.input_len.iter().collect();
    ins.sort();
    let mut outs: Vec<_> = hid.output_len.iter().collect();
    outs.sort();
    println!("input reports (id: bytes): {ins:02x?}");
    println!("output reports (id: bytes): {outs:02x?}");
    println!("press buttons, turn knobs, hit pads... (Ctrl-C to quit)\n");
    let mut prev: Option<Vec<u8>> = None;
    let mut buf = [0u8; 512];
    let mut pfd = pollfd { fd: hid.fd(), events: POLLIN, revents: 0 };
    loop {
        // SAFETY: one valid pollfd.
        if unsafe { libc::poll(&mut pfd, 1, -1) } < 0 {
            continue;
        }
        while let Some(r) = hid.read(&mut buf)? {
            println!("[{:3} bytes] {}", r.len(), hex(r));
            match r[0] {
                REPORT_BUTTONS if r.len() > STRIP_OFFSET + 1 => {
                    if let Some(p) = &prev {
                        for b in BUTTONS {
                            let (now, was) = (r[b.byte] & b.mask != 0, p[b.byte] & b.mask != 0);
                            if now != was {
                                println!("    {} {}", b.name, if now { "PRESSED" } else { "released" });
                            }
                        }
                    }
                    let knobs: Vec<u16> =
                        (0..8).map(|k| u16::from_le_bytes([r[KNOBS_OFFSET + 2 * k], r[KNOBS_OFFSET + 2 * k + 1]])).collect();
                    println!(
                        "    encoder={} knobs={:?} strip={} pedal={}",
                        r[ENCODER_BYTE] & 0x0f,
                        knobs,
                        u16::from_le_bytes([r[STRIP_OFFSET], r[STRIP_OFFSET + 1]]),
                        r[PEDAL_BYTE] & PEDAL_MASK != 0
                    );
                    prev = Some(r.to_vec());
                }
                REPORT_PADS => {
                    for chunk in r.chunks(64) {
                        let mut i = 1;
                        while i + 2 < chunk.len() {
                            let (idx, hi, lo) = (chunk[i], chunk[i + 1], chunk[i + 2]);
                            i += 3;
                            if idx == 0 && hi == 0 && lo == 0 {
                                break;
                            }
                            let v = ((hi as u16 & 0xf) << 8) | lo as u16;
                            let note = if idx < 16 { pad_note(idx as usize) } else { 0 };
                            println!("    pad idx={idx:2} (note {note}) event={} value={v}", hi >> 4);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn led_test(value: Option<String>) -> Result<(), Box<dyn Error>> {
    let v = match value {
        Some(s) => parse_num(Some(s))? as u8,
        None => 0x7e,
    };
    let mut hid = Hidraw::open()?;
    let pause = || std::thread::sleep(Duration::from_millis(350));
    println!("button LEDs (report 0x80), value 0x{v:02x}:");
    for i in 0..BUTTON_LEDS {
        let mut b = [0u8; BUTTON_LEDS];
        b[i] = v;
        hid.write_report(REPORT_LEDS_BUTTONS, &b, 63)?;
        let names: Vec<_> = BUTTONS
            .iter()
            .filter(|b| matches!(b.led, protocol::Led::Mono(n) | protocol::Led::Color(n) | protocol::Led::ColorOnOff(n) if n == i))
            .map(|b| b.name)
            .collect();
        println!("  LED {i:2} -> expected: {}", if names.is_empty() { "?".into() } else { names.join(", ") });
        pause();
    }
    hid.write_report(REPORT_LEDS_BUTTONS, &[0; BUTTON_LEDS], 63)?;
    println!("pad LEDs (report 0x81): pads 1..16 in order, colour 1 (red)");
    for n in 0..16u8 {
        let mut p = [0u8; PAD_LED_BYTES];
        p[PAD_LEDS_OFFSET + note_pad(PAD_NOTE_BASE + n).unwrap()] = 4 + 2;
        hid.write_report(REPORT_LEDS_PADS, &p, 63)?;
        println!("  Pad {}", n + 1);
        pause();
    }
    println!("all pads, the 17 colours x 4 brightness levels (value = colour*4 + brightness)");
    for c in 1..=17u8 {
        for br in 0..4u8 {
            let mut p = [0u8; PAD_LED_BYTES];
            p[PAD_LEDS_OFFSET..].fill(c * 4 + br);
            hid.write_report(REPORT_LEDS_PADS, &p, 63)?;
            std::thread::sleep(Duration::from_millis(120));
        }
        println!("  colour {c}");
    }
    println!("touch strip LEDs, left to right");
    for i in 0..STRIP_LEDS {
        let mut p = [0u8; PAD_LED_BYTES];
        p[i] = 17 * 4 + 2;
        hid.write_report(REPORT_LEDS_PADS, &p, 63)?;
        std::thread::sleep(Duration::from_millis(80));
    }
    hid.write_report(REPORT_LEDS_PADS, &[0; PAD_LED_BYTES], 63)?;
    println!("done");
    Ok(())
}

fn screen_test() -> Result<(), Box<dyn Error>> {
    let mut s = Screens::open().map_err(|e| format!("can't open the displays' USB interface: {e}"))?;
    for i in 0..2u8 {
        s.show(i, &screen::test_pattern(i as usize))?;
    }
    println!("test pattern sent to both displays");
    Ok(())
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    let result = match args.peek().map(String::as_str) {
        Some("dump") => dump(),
        Some("led-test") => {
            args.next();
            led_test(args.next())
        }
        Some("screen-test") => screen_test(),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            if args.peek().map(String::as_str) == Some("run") {
                args.next();
            }
            match parse_opts(args) {
                Ok(o) => run(o),
                Err(e) => {
                    eprintln!("maschine-mk3: {e}\n\n{USAGE}");
                    return ExitCode::from(2);
                }
            }
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("maschine-mk3: {e}");
            if e.downcast_ref::<io::Error>().is_some_and(|e| e.kind() == io::ErrorKind::NotFound) {
                ExitCode::from(3) // no device: systemd won't restart (RestartPreventExitStatus=3)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
