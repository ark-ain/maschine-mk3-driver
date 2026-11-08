//! The two 480x272 colour displays: draw Bitwig's text, encode, and send over USB bulk.
//!
//! Wire format (from OpenAV Ctlra): a 16-byte header (`84 00 <screen> 60`, 8 zero bytes,
//! width and height as big-endian u16), then commands, then `03 00 00 00 40 00 00 00`.
//! Command `00 00 <n BE16>` is followed by 2*n pixels of RGB565 (big-endian).

use std::convert::Infallible;
use std::time::Duration;

use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X10};
use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::raw::RawU16;
use embedded_graphics::pixelcolor::Rgb565 as EgRgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};

use crate::protocol::*;

pub type Rgb565 = u16;

pub const fn rgb(r: u8, g: u8, b: u8) -> Rgb565 {
    ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3)
}

pub const BLACK: Rgb565 = rgb(0, 0, 0);
pub const WHITE: Rgb565 = rgb(255, 255, 255);
pub const GREY: Rgb565 = rgb(110, 110, 110);
pub const ORANGE: Rgb565 = rgb(255, 150, 40);
pub const DIM: Rgb565 = rgb(45, 45, 45);
pub const CYAN: Rgb565 = rgb(90, 210, 255);

/// Knob columns: 4 per screen, 120 px each, one Bitwig field of 7 chars each.
const COL_W: usize = SCREEN_W / 4;
const FIELD: usize = DISPLAY_CELL / 4;

pub struct Frame {
    pub px: Vec<Rgb565>,
}

impl OriginDimensions for Frame {
    fn size(&self) -> Size {
        Size::new(SCREEN_W as u32, SCREEN_H as u32)
    }
}

impl DrawTarget for Frame {
    type Color = EgRgb565;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<EgRgb565>>>(&mut self, pixels: I) -> Result<(), Infallible> {
        for Pixel(p, c) in pixels {
            if p.x >= 0 && p.y >= 0 && (p.x as usize) < SCREEN_W && (p.y as usize) < SCREEN_H {
                self.px[p.y as usize * SCREEN_W + p.x as usize] = RawU16::from(c).into_inner();
            }
        }
        Ok(())
    }
}

impl Frame {
    pub fn new(fill: Rgb565) -> Frame {
        Frame { px: vec![fill; SCREEN_W * SCREEN_H] }
    }

    pub fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: Rgb565) {
        for yy in y..(y + h).min(SCREEN_H) {
            for xx in x..(x + w).min(SCREEN_W) {
                self.px[yy * SCREEN_W + xx] = c;
            }
        }
    }

    /// Text with its top at `y`; `x` is the left edge, or the centre if `center`.
    pub fn text(&mut self, s: &str, x: i32, y: i32, font: &MonoFont, c: Rgb565, center: bool) {
        let style = MonoTextStyle::new(font, EgRgb565::from(RawU16::new(c)));
        let layout = TextStyleBuilder::new()
            .alignment(if center { Alignment::Center } else { Alignment::Left })
            .baseline(Baseline::Top)
            .build();
        let _ = Text::with_text_style(s, Point::new(x, y), style, layout).draw(self);
    }

    /// The USB transfer for screen 0 (left) or 1 (right).
    #[cfg(test)]
    pub fn encode(&self, screen: u8) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode_into(screen, &mut out);
        out
    }

    /// Same, into a reused buffer (one transfer is ~261 KB).
    pub fn encode_into(&self, screen: u8, out: &mut Vec<u8>) {
        let n = SCREEN_W * SCREEN_H;
        out.clear();
        out.reserve(16 + 4 + n * 2 + 8);
        out.extend_from_slice(&[0x84, 0x00, screen, 0x60, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&(SCREEN_W as u16).to_be_bytes());
        out.extend_from_slice(&(SCREEN_H as u16).to_be_bytes());
        out.extend_from_slice(&[0x00, 0x00]);
        out.extend_from_slice(&((n / 2) as u16).to_be_bytes());
        for p in &self.px {
            out.extend_from_slice(&p.to_be_bytes());
        }
        out.extend_from_slice(&[0x03, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00]);
    }
}

/// Bitwig sends a 28-char line either as free text ("P: Peak Limiter") or as 4 fields of 7
/// chars, one per knob, separated by '|' ("Gain  |Ceilin|Releas|Releas").
fn is_fields(line: &[u8]) -> bool {
    line.len() >= 3 * FIELD && (1..4).all(|k| line[k * FIELD - 1] == b'|')
}

fn draw_line(f: &mut Frame, line: &[u8], y: i32, c: Rgb565) {
    if is_fields(line) {
        // one field centred under each knob, with a thin separator between columns
        for k in 0..4 {
            let field = String::from_utf8_lossy(&line[k * FIELD..(k + 1) * FIELD]);
            let field = field.trim_end_matches('|').trim();
            f.text(field, (k * COL_W + COL_W / 2) as i32, y, &FONT_10X20, c, true);
            if k > 0 {
                f.rect(k * COL_W, y as usize - 2, 1, 24, DIM);
            }
        }
    } else {
        let text = String::from_utf8_lossy(line);
        f.text(text.trim_end(), 14, y, &FONT_10X20, c, false);
    }
}

/// One screen in MIDI mode: Bitwig's two 28-char lines for this half of the display.
/// Left screen = cells 0 (top) and 2 (bottom); right screen = cells 1 and 3.
pub fn render(display: &[u8; DISPLAY_TOTAL], screen: usize) -> Frame {
    let mut f = Frame::new(BLACK);
    let top = &display[screen * DISPLAY_CELL..(screen + 1) * DISPLAY_CELL];
    let bottom = &display[(2 + screen) * DISPLAY_CELL..(3 + screen) * DISPLAY_CELL];
    let label = if screen == 0 { "MIDI MODE" } else { "BITWIG" };
    f.text(label, 14, 8, &FONT_6X10, GREY, false);
    draw_line(&mut f, top, 60, WHITE);
    f.rect(14, 110, SCREEN_W - 28, 1, DIM);
    draw_line(&mut f, bottom, 140, ORANGE);
    // Values of the 4 knobs of this screen, each under its knob (Linux extension); without
    // them, a small mark over each knob.
    let row = &display[DISPLAY_CHARS + screen * VALUES_ROW..DISPLAY_CHARS + (screen + 1) * VALUES_ROW];
    let values = String::from_utf8_lossy(row);
    if values.trim().is_empty() {
        for k in 0..4 {
            f.rect(k * COL_W + COL_W / 2 - 20, 254, 40, 3, GREY);
        }
    } else {
        f.rect(14, 200, SCREEN_W - 28, 1, DIM);
        for (k, v) in values.split('|').take(4).enumerate() {
            f.text(v.trim(), (k * COL_W + COL_W / 2) as i32, 225, &FONT_10X20, CYAN, true);
            if k > 0 {
                f.rect(k * COL_W, 223, 1, 24, DIM);
            }
        }
    }
    f
}

pub fn blank() -> Frame {
    Frame::new(BLACK)
}

/// Test pattern: colour bars, screen number and a few strings.
pub fn test_pattern(screen: usize) -> Frame {
    let mut f = Frame::new(BLACK);
    let bars = [
        rgb(255, 0, 0),
        rgb(0, 255, 0),
        rgb(0, 0, 255),
        rgb(255, 255, 0),
        rgb(0, 255, 255),
        rgb(255, 0, 255),
        WHITE,
        GREY,
    ];
    for (i, c) in bars.iter().enumerate() {
        f.rect(i * 60, 0, 60, 120, *c);
    }
    f.text(&format!("SCREEN {screen}"), 20, 140, &FONT_10X20, WHITE, false);
    f.text("R G B Y C M W grey", 20, 180, &FONT_10X20, ORANGE, false);
    f.text("Maschine MK3 Linux driver", 20, 220, &FONT_10X20, GREY, false);
    f
}

/// The displays' USB interface, through libusb.
pub struct Screens {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    buf: Vec<u8>,
}

impl Screens {
    pub fn open() -> rusb::Result<Screens> {
        let handle = rusb::open_device_with_vid_pid(VID, PID).ok_or(rusb::Error::NoDevice)?;
        // usually no kernel driver on this interface; detach one if there is
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle.claim_interface(SCREEN_INTERFACE)?;
        Ok(Screens { handle, buf: Vec::new() })
    }

    pub fn show(&mut self, screen: u8, frame: &Frame) -> rusb::Result<()> {
        frame.encode_into(screen, &mut self.buf);
        let mut off = 0;
        while off < self.buf.len() {
            match self.handle.write_bulk(SCREEN_ENDPOINT, &self.buf[off..], Duration::from_secs(2))? {
                0 => return Err(rusb::Error::Io),
                n => off += n,
            }
        }
        Ok(())
    }
}

/// What the displays should show.
pub enum Cmd {
    /// Bitwig's text, and whether MIDI mode is on (off = blank).
    Show(Box<[u8; DISPLAY_TOTAL]>, bool),
    /// Blank both displays and stop.
    Quit,
}

/// Drives the displays from their own thread: a USB transfer takes ~10-20 ms, which must not
/// delay pads and buttons. Only the latest state counts (older pending ones are skipped), only
/// the screen whose text changed is redrawn, at most 25 times per second.
pub fn spawn(mut screens: Screens) -> (std::sync::mpsc::Sender<Cmd>, std::thread::JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::channel::<Cmd>();
    let thread = std::thread::spawn(move || {
        let mut shown: [Option<(Vec<u8>, bool)>; 2] = [None, None];
        let mut cmd = rx.recv();
        loop {
            // keep only the most recent command
            while let Ok(newer) = rx.try_recv() {
                if !matches!(cmd, Ok(Cmd::Quit)) {
                    cmd = Ok(newer);
                }
            }
            match cmd {
                Ok(Cmd::Show(text, on)) => {
                    for i in 0..2 {
                        let row = DISPLAY_CHARS + i * VALUES_ROW;
                        let mine = [&text[i * DISPLAY_CELL..(i + 1) * DISPLAY_CELL],
                            &text[(2 + i) * DISPLAY_CELL..(3 + i) * DISPLAY_CELL],
                            &text[row..row + VALUES_ROW]]
                            .concat();
                        let state = Some((mine, on));
                        if shown[i] == state {
                            continue;
                        }
                        let f = if on { render(&text, i) } else { blank() };
                        if let Err(e) = screens.show(i as u8, &f) {
                            eprintln!("maschine-mk3: display write failed ({e}); disabling displays");
                            return;
                        }
                        shown[i] = state;
                    }
                }
                Ok(Cmd::Quit) | Err(_) => {
                    let _ = (0..2).try_for_each(|i| screens.show(i, &blank()));
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(40));
            cmd = rx.recv();
        }
    });
    (tx, thread)
}

impl Drop for Screens {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(SCREEN_INTERFACE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_layout() {
        let p = blank().encode(1);
        assert_eq!(&p[..4], &[0x84, 0x00, 0x01, 0x60]);
        assert_eq!(&p[12..16], &[0x01, 0xe0, 0x01, 0x10]);
        assert_eq!(&p[16..20], &[0x00, 0x00, 0xff, 0x00]);
        assert_eq!(p.len(), 16 + 4 + 480 * 272 * 2 + 8);
        assert_eq!(&p[p.len() - 8..], &[0x03, 0, 0, 0, 0x40, 0, 0, 0]);
    }

    #[test]
    fn text_draws_pixels() {
        let mut d = [b' '; DISPLAY_TOTAL];
        d[..5].copy_from_slice(b"Hello");
        let f = render(&d, 0);
        let lit = f.px.iter().filter(|&&p| p == WHITE).count();
        assert!(lit > 0);
        assert_eq!(rgb(255, 255, 255), 0xffff);
    }

    #[test]
    fn fields() {
        assert!(is_fields(b"Gain  |Ceilin|Releas|Releas"));
        assert!(is_fields(b" ---- | ---- |<PARAM|PARAM>"));
        assert!(!is_fields(b"P: Peak Limiter             "));
    }
}

#[cfg(test)]
mod preview {
    /// `PREVIEW_PPM=/path/out.ppm cargo test preview -- --ignored` writes both screens side by side.
    #[test]
    #[ignore]
    fn preview() {
        use super::*;
        let mut d = [b' '; DISPLAY_TOTAL];
        let put = |d: &mut [u8; DISPLAY_TOTAL], off: usize, s: &[u8]| d[off..off + s.len()].copy_from_slice(s);
        put(&mut d, 0, b"P: Peak Limiter");
        put(&mut d, 28, b" ---- | ---- |<PARAM|PARAM>");
        put(&mut d, 56, b"Gain  |Ceilin|Releas|Releas");
        put(&mut d, 84, b"Volume = -3.2 dB");
        put(&mut d, DISPLAY_CHARS, b"0.00 dB|-6.0 dB|25.0 %|1.20 s");
        let (l, r) = (render(&d, 0), render(&d, 1));
        let mut out = format!("P6 {} {} 255\n", SCREEN_W * 2 + 16, SCREEN_H).into_bytes();
        for y in 0..SCREEN_H {
            for x in 0..SCREEN_W * 2 + 16 {
                let p = if x < SCREEN_W { l.px[y * SCREEN_W + x] } else if x >= SCREEN_W + 16 { r.px[y * SCREEN_W + x - SCREEN_W - 16] } else { 0x4208 };
                out.extend_from_slice(&[((p >> 11) << 3) as u8, (((p >> 5) & 63) << 2) as u8, ((p & 31) << 3) as u8]);
            }
        }
        std::fs::write(std::env::var("PREVIEW_PPM").unwrap(), out).unwrap();
    }
}
