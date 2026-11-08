//! HID ↔ MIDI translation, the way NI's MIDI mode does it for Bitwig's "Maschine Mk3" extension.
//! Pure logic, no I/O: `main.rs` feeds it HID reports and MIDI messages.

use crate::protocol::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PadMode {
    /// Use the note-on/note-off events in the pad report (NI firmware, as on the Mikro MK3).
    Events,
    /// Ignore the events and derive notes from pressure crossing a threshold (as Ctlra does).
    Threshold,
}

#[derive(Clone, Debug)]
pub struct Config {
    /// LED value for an enabled, idle button (backlight).
    pub led_dim: u8,
    /// LED value for a lit button.
    pub led_on: u8,
    /// Colour index used for RGB LEDs that Bitwig drives with on/off (white, bright).
    pub led_white: u8,
    /// Touch strip LED value when lit.
    pub strip_on: u8,
    /// Raw knob units (1000 per turn) per MIDI step. Smaller = faster. Four times finer while
    /// SHIFT is held.
    pub knob_divider: i32,
    /// Extra raw units needed to reverse a knob's direction (filters sensor jitter).
    pub knob_hysteresis: i32,
    pub pad_mode: PadMode,
    /// Threshold mode: raw pressure (0..4095) above which a pad is "hit".
    pub pad_threshold: u16,
    /// Raw pad value (0..4095) that gives velocity 127. Real hits measure ~150..2500.
    pub pad_vel_max: u16,
    /// Velocity curve exponent: 1 = linear, < 1 = soft hits louder.
    pub pad_curve: f32,
    /// A light touch (the firmware's own "no note") whose pressure reaches this still plays
    /// a note. 0 = never.
    pub pad_touch_trigger: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            led_dim: 0x7c,
            led_on: 0x7e,
            led_white: 17 * 4 + 2,
            strip_on: 17 * 4 + 2,
            knob_divider: 13,
            knob_hysteresis: 6,
            pad_mode: PadMode::Events,
            pad_threshold: 128,
            pad_vel_max: 2048,
            pad_curve: 0.6,
            pad_touch_trigger: 70,
        }
    }
}

impl Config {
    /// 12-bit pad value → 7-bit MIDI value, through the velocity curve.
    pub fn curve(&self, v: u16) -> u8 {
        let x = (v as f32 / self.pad_vel_max.max(1) as f32).clamp(0.0, 1.0);
        (x.powf(self.pad_curve.max(0.05)) * 127.0).round() as u8
    }
}

pub struct Translator {
    cfg: Config,
    /// Previous 0x01 report (`None` until the first one: it only sets the baseline).
    prev: Option<Vec<u8>>,
    /// Buttons whose press CC was sent, so the release is sent only for those.
    sent: Vec<bool>,
    knob_accum: [i32; 8],
    /// Direction of the last step sent per knob (-1, 0, 1).
    knob_dir: [i32; 8],
    last_strip_cc: Option<u8>,
    pad_on: [bool; 16],
    /// Pad is in a firmware "light touch" (event 0 .. event 2).
    pad_touch: [bool; 16],
    pad_last_at: [u8; 16],
    shift: bool,
    pub midi_mode: bool,
    /// LED state built from Bitwig's MIDI (kept while MIDI mode is off).
    btn_leds: [u8; BUTTON_LEDS],
    pad_leds: [u8; PAD_LED_BYTES],
    /// Display text from Bitwig: 112 chars (4 cells of 28).
    pub display: [u8; DISPLAY_TOTAL],
    pub display_dirty: bool,
    /// Partial sysex being reassembled.
    sysex: Vec<u8>,
}

impl Translator {
    pub fn new(cfg: Config) -> Translator {
        let mut btn_leds = [0; BUTTON_LEDS];
        for b in BUTTONS {
            if let Led::Mono(i) = b.led {
                btn_leds[i] = cfg.led_dim;
            }
        }
        Translator {
            cfg,
            prev: None,
            sent: vec![false; BUTTONS.len()],
            knob_accum: [0; 8],
            knob_dir: [0; 8],
            last_strip_cc: None,
            pad_on: [false; 16],
            pad_touch: [false; 16],
            pad_last_at: [0; 16],
            shift: false,
            midi_mode: true,
            btn_leds,
            pad_leds: [0; PAD_LED_BYTES],
            display: [b' '; DISPLAY_TOTAL],
            display_dirty: true,
            sysex: Vec::new(),
        }
    }

    /// Initial state of the buttons/knobs (GET_REPORT of input 0x01), without emitting MIDI.
    pub fn init(&mut self, report: &[u8]) {
        if report.first() == Some(&REPORT_BUTTONS) && report.len() >= STRIP_OFFSET + 2 {
            self.prev = Some(report.to_vec());
        }
    }

    /// One HID input report (ID byte first) → MIDI messages for Bitwig.
    pub fn input(&mut self, report: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        match report.first() {
            Some(&REPORT_BUTTONS) => self.buttons(report, &mut out),
            Some(&REPORT_PADS) => {
                // hidraw normally gives one 64-byte report; a 128-byte read holds two (Ctlra).
                for chunk in report.chunks(64) {
                    if chunk[0] == REPORT_PADS {
                        self.pads(chunk, &mut out);
                    }
                }
            }
            _ => {}
        }
        out
    }

    fn buttons(&mut self, r: &[u8], out: &mut Vec<Vec<u8>>) {
        if r.len() < STRIP_OFFSET + 2 {
            return;
        }
        let prev = match self.prev.replace(r.to_vec()) {
            Some(p) => p,
            // No initial state (GET_REPORT failed): the first report is only sent because
            // something changed. Knobs, encoder and strip take it as their baseline (their first
            // absolute values can be anything); buttons are compared against "all released",
            // so the press that produced this report isn't lost.
            None => {
                let mut p = r.to_vec();
                p[1..ENCODER_BYTE].fill(0);
                p
            }
        };
        if prev.len() != r.len() {
            return;
        }

        // Buttons
        for (i, b) in BUTTONS.iter().enumerate() {
            let now = r[b.byte] & b.mask != 0;
            let was = prev[b.byte] & b.mask != 0;
            if now == was {
                continue;
            }
            match b.action {
                Action::Shift => {
                    self.shift = now;
                    if self.midi_mode {
                        out.push(if now { SYSEX_SHIFT_DOWN } else { SYSEX_SHIFT_UP }.to_vec());
                    }
                }
                Action::Cc(cc) => {
                    if now && self.shift && b.name == MIDI_MODE_TOGGLE {
                        if self.midi_mode {
                            // Leaving MIDI mode: close everything Bitwig thinks is still held,
                            // otherwise it stays in its SHIFT layer or keeps notes hanging.
                            out.push(SYSEX_SHIFT_UP.to_vec());
                            for p in 0..16 {
                                self.pad_touch[p] = false;
                                if std::mem::take(&mut self.pad_on[p]) {
                                    out.push(vec![NOTE_OFF, pad_note(p), 0]);
                                }
                            }
                            if prev[STRIP_OFFSET] | prev[STRIP_OFFSET + 1] != 0 {
                                out.push(vec![CC, CC_STRIP_TOUCH, 0]);
                            }
                        }
                        self.midi_mode = !self.midi_mode;
                        self.display_dirty = true;
                        if self.midi_mode {
                            out.push(SYSEX_RETURN_FROM_HOST.to_vec());
                            // SHIFT is still held: tell Bitwig, so its later release is paired.
                            out.push(SYSEX_SHIFT_DOWN.to_vec());
                        }
                        continue;
                    }
                    if now {
                        if self.midi_mode {
                            out.push(vec![CC, cc, 127]);
                            self.sent[i] = true;
                        }
                    } else if std::mem::take(&mut self.sent[i]) {
                        out.push(vec![CC, cc, 0]);
                    }
                }
            }
        }
        if !self.midi_mode {
            return;
        }

        // Main encoder: 4-bit position, one detent = 1.
        let (e1, e0) = (r[ENCODER_BYTE] & 0x0f, prev[ENCODER_BYTE] & 0x0f);
        if e1 != e0 {
            let d = ((e1 as i8 - e0 as i8 + 8).rem_euclid(16)) - 8;
            if d != 0 {
                out.push(vec![CC, CC_ENCODER_TURN, (d as u8) & 0x7f]);
            }
        }

        // 8 knobs: absolute 0..999 → relative two's complement CC 70..77.
        for k in 0..8 {
            let o = KNOBS_OFFSET + 2 * k;
            let v1 = u16::from_le_bytes([r[o], r[o + 1]]) as i32;
            let v0 = u16::from_le_bytes([prev[o], prev[o + 1]]) as i32;
            let mut d = v1 - v0;
            if d > KNOB_RANGE / 2 {
                d -= KNOB_RANGE;
            } else if d < -KNOB_RANGE / 2 {
                d += KNOB_RANGE;
            }
            // The sensors jitter by a few units even untouched (knob 8 measured 677..682).
            // Movement accumulates from where the last step was sent; the remainder is dropped
            // after each step, and going back the other way needs `knob_hysteresis` more.
            self.knob_accum[k] += d;
            let a = self.knob_accum[k];
            let div = self.cfg.knob_divider.max(1) * if self.shift { 4 } else { 1 };
            let reversing = self.knob_dir[k] != 0 && a.signum() != self.knob_dir[k];
            let need = if reversing { div + self.cfg.knob_hysteresis } else { div };
            if a.abs() >= need {
                let steps = (a / div).clamp(-63, 63);
                self.knob_accum[k] = 0;
                self.knob_dir[k] = a.signum();
                out.push(vec![CC, CC_KNOB_BASE + k as u8, (steps as i8 as u8) & 0x7f]);
            }
        }

        // Touch strip: CC 2 = touch, CC 1 = position.
        let s1 = u16::from_le_bytes([r[STRIP_OFFSET], r[STRIP_OFFSET + 1]]) as u32;
        let s0 = u16::from_le_bytes([prev[STRIP_OFFSET], prev[STRIP_OFFSET + 1]]) as u32;
        if s0 == 0 && s1 != 0 {
            out.push(vec![CC, CC_STRIP_TOUCH, 127]);
        }
        if s1 != 0 && s1 != s0 {
            let v = ((s1.min(STRIP_MAX) * 127 + STRIP_MAX / 2) / STRIP_MAX) as u8;
            if self.last_strip_cc != Some(v) {
                self.last_strip_cc = Some(v);
                out.push(vec![CC, CC_STRIP, v]);
            }
        }
        if s0 != 0 && s1 == 0 {
            out.push(vec![CC, CC_STRIP_TOUCH, 0]);
        }

        // Footswitch
        let (p1, p0) = (r[PEDAL_BYTE] & PEDAL_MASK != 0, prev[PEDAL_BYTE] & PEDAL_MASK != 0);
        if p1 != p0 {
            out.push(vec![CC, CC_PEDAL, if p1 { 127 } else { 0 }]);
        }
    }

    fn pads(&mut self, r: &[u8], out: &mut Vec<Vec<u8>>) {
        let mut i = 1;
        while i + 2 < r.len() {
            let (idx, hi, lo) = (r[i] as usize, r[i + 1], r[i + 2]);
            i += 3;
            if idx == 0 && hi == 0 && lo == 0 {
                break; // end of list
            }
            if idx >= 16 {
                continue;
            }
            let event = hi >> 4;
            let value = ((hi as u16 & 0x0f) << 8) | lo as u16;
            if !self.midi_mode {
                self.pad_on[idx] = false;
                self.pad_touch[idx] = false;
                continue;
            }
            let note = pad_note(idx);
            match self.cfg.pad_mode {
                PadMode::Events => match event {
                    1 => self.note_on(idx, note, value, out),
                    3 => self.note_off(idx, note, out),
                    // Light touch: the firmware sends no note-on. Soft hits on worn pads land
                    // here, so play a note once the pressure reaches `pad_touch_trigger`.
                    0 => {
                        self.pad_touch[idx] = true;
                        self.touch_trigger(idx, note, value, out);
                    }
                    2 => {
                        if std::mem::take(&mut self.pad_touch[idx]) {
                            self.note_off(idx, note, out);
                        }
                    }
                    4 => {
                        if self.pad_touch[idx] && !self.pad_on[idx] {
                            self.touch_trigger(idx, note, value, out);
                        } else {
                            self.pressure(idx, note, value, out);
                        }
                    }
                    _ => {}
                },
                PadMode::Threshold => {
                    let hit = value > self.cfg.pad_threshold;
                    if hit && !self.pad_on[idx] {
                        self.note_on(idx, note, value, out);
                    } else if !hit && self.pad_on[idx] {
                        self.note_off(idx, note, out);
                    } else {
                        self.pressure(idx, note, value, out);
                    }
                }
            }
        }
    }

    fn note_on(&mut self, idx: usize, note: u8, value: u16, out: &mut Vec<Vec<u8>>) {
        if self.pad_on[idx] {
            return;
        }
        self.pad_on[idx] = true;
        self.pad_last_at[idx] = 0;
        out.push(vec![NOTE_ON, note, self.cfg.curve(value).max(1)]);
    }

    fn touch_trigger(&mut self, idx: usize, note: u8, value: u16, out: &mut Vec<Vec<u8>>) {
        let t = self.cfg.pad_touch_trigger;
        if t > 0 && value >= t {
            self.note_on(idx, note, value, out);
        }
    }

    fn note_off(&mut self, idx: usize, note: u8, out: &mut Vec<Vec<u8>>) {
        if std::mem::take(&mut self.pad_on[idx]) {
            out.push(vec![NOTE_OFF, note, 0]);
        }
    }

    fn pressure(&mut self, idx: usize, note: u8, value: u16, out: &mut Vec<Vec<u8>>) {
        let v = self.cfg.curve(value);
        if self.pad_on[idx] && v != self.pad_last_at[idx] {
            self.pad_last_at[idx] = v;
            out.push(vec![POLY_AT, note, v]);
        }
    }

    /// One MIDI message (or sysex fragment) from Bitwig → LED and display state.
    pub fn midi(&mut self, msg: &[u8]) {
        if msg.is_empty() {
            return;
        }
        // Sysex, possibly split over several sequencer events.
        let continuation = !self.sysex.is_empty() && (msg[0] < 0x80 || msg[0] == 0xF7);
        if !continuation {
            self.sysex.clear(); // an unfinished sysex followed by something else: drop it
        }
        if msg[0] == 0xF0 || continuation {
            if msg[0] == 0xF0 {
                self.sysex.clear();
            }
            self.sysex.extend_from_slice(msg);
            if self.sysex.last() == Some(&0xF7) {
                let s = std::mem::take(&mut self.sysex);
                self.sysex_in(&s);
            } else if self.sysex.len() > 1024 {
                self.sysex.clear();
            }
            return;
        }
        if msg.len() < 3 || msg[0] & 0x0f != 0 {
            return; // channel 1 only
        }
        let (status, d1, d2) = (msg[0] & 0xf0, msg[1], msg[2]);
        match status {
            CC => {
                if d1 == CC_STRIP {
                    let n = ((d2 as usize * STRIP_LEDS) + 63) / 127;
                    for i in 0..STRIP_LEDS {
                        self.pad_leds[i] = if i < n { self.cfg.strip_on } else { 0 };
                    }
                }
                for b in BUTTONS {
                    if b.action != Action::Cc(d1) {
                        continue;
                    }
                    match b.led {
                        Led::Mono(i) => self.btn_leds[i] = if d2 > 0 { self.cfg.led_on } else { self.cfg.led_dim },
                        Led::Color(i) => self.btn_leds[i] = d2,
                        Led::ColorOnOff(i) => self.btn_leds[i] = if d2 > 0 { self.cfg.led_white } else { 0 },
                        Led::None => {}
                    }
                }
            }
            NOTE_ON | NOTE_OFF => {
                if let Some(p) = note_pad(d1) {
                    self.pad_leds[PAD_LEDS_OFFSET + p] = if status == NOTE_ON { d2 } else { 0 };
                }
            }
            _ => {}
        }
    }

    fn sysex_in(&mut self, s: &[u8]) {
        // F0 00 00 66 <dev> 12 <offset> <chars…> F7
        if s.len() >= 8 && s[..4] == SYSEX_DISPLAY_PREFIX && s[5] == 0x12 {
            let off = s[6] as usize;
            for (j, &c) in s[7..s.len() - 1].iter().enumerate() {
                if off + j < DISPLAY_CHARS {
                    self.display[off + j] = if (0x20..0x7f).contains(&c) { c } else { b' ' };
                }
            }
            self.display_dirty = true;
        } else if s.len() >= 8 && s[..4] == SYSEX_DISPLAY_PREFIX && s[5] == SYSEX_VALUES_CMD {
            let screen = s[6] as usize;
            if screen < 2 {
                let row = DISPLAY_CHARS + screen * VALUES_ROW;
                self.display[row..row + VALUES_ROW].fill(b' ');
                for (j, &c) in s[7..s.len() - 1].iter().take(VALUES_ROW).enumerate() {
                    self.display[row + j] = if (0x20..0x7f).contains(&c) { c } else { b' ' };
                }
                self.display_dirty = true;
            }
        }
    }

    /// What the button LEDs (report 0x80) and strip+pad LEDs (report 0x81) should show now.
    pub fn leds(&self) -> ([u8; BUTTON_LEDS], [u8; PAD_LED_BYTES]) {
        if !self.midi_mode {
            return ([0; BUTTON_LEDS], [0; PAD_LED_BYTES]);
        }
        let mut btn = self.btn_leds;
        for b in BUTTONS {
            if let (Action::Shift, Led::Mono(i)) = (b.action, b.led) {
                btn[i] = if self.shift { self.cfg.led_on } else { self.cfg.led_dim };
            }
        }
        (btn, self.pad_leds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Vec<u8> {
        let mut r = vec![0u8; 42];
        r[0] = REPORT_BUTTONS;
        r
    }

    fn btn(name: &str) -> &'static Button {
        BUTTONS.iter().find(|b| b.name == name).unwrap()
    }

    fn started() -> Translator {
        let mut t = Translator::new(Config::default());
        assert!(t.input(&report()).is_empty()); // baseline
        t
    }

    #[test]
    fn play_press_release() {
        let mut t = started();
        let play = btn("Play");
        let mut r = report();
        r[play.byte] |= play.mask;
        assert_eq!(t.input(&r), vec![vec![0xB0, 57, 127]]);
        assert_eq!(t.input(&report()), vec![vec![0xB0, 57, 0]]);
    }

    #[test]
    fn first_report_without_init() {
        // buttons: the press that caused the first report is sent
        let mut t = Translator::new(Config::default());
        let mut r = report();
        r[btn("Play").byte] |= btn("Play").mask;
        r[KNOBS_OFFSET] = 0x37; // random-looking absolute knob value: baseline, no CC
        r[ENCODER_BYTE] = 9;
        assert_eq!(t.input(&r), vec![vec![0xB0, 57, 127]]);
        // with init (GET_REPORT), a button already held isn't re-sent
        let mut t = Translator::new(Config::default());
        t.init(&r);
        assert!(t.input(&r).is_empty());
    }

    #[test]
    fn leaving_midi_mode_releases_notes() {
        let mut t = started();
        let mut p = vec![0u8; 64];
        p[0] = REPORT_PADS;
        p[1..4].copy_from_slice(&[0, 0x1f, 0xff]); // pad idx 0 = note 72
        assert_eq!(t.input(&p), vec![vec![0x90, 72, 127]]);
        let (sh, ch) = (btn("Shift"), btn("Channel"));
        let mut r = report();
        r[sh.byte] |= sh.mask;
        t.input(&r);
        r[ch.byte] |= ch.mask;
        assert_eq!(t.input(&r), vec![SYSEX_SHIFT_UP.to_vec(), vec![0x80, 72, 0]]);
    }

    #[test]
    fn shift_sysex_and_midi_mode_toggle() {
        let mut t = started();
        let (sh, ch) = (btn("Shift"), btn("Channel"));
        let mut r = report();
        r[sh.byte] |= sh.mask;
        assert_eq!(t.input(&r), vec![SYSEX_SHIFT_DOWN.to_vec()]);
        assert_eq!(t.leds().0[44], 0x7e);
        r[ch.byte] |= ch.mask;
        assert_eq!(t.input(&r), vec![SYSEX_SHIFT_UP.to_vec()]); // toggles off, no CC 34
        assert!(!t.midi_mode);
        assert_eq!(t.leds().0, [0; BUTTON_LEDS]);
        r[ch.byte] &= !ch.mask;
        assert!(t.input(&r).is_empty()); // no unpaired release
        r[ch.byte] |= ch.mask;
        assert_eq!(t.input(&r), vec![SYSEX_RETURN_FROM_HOST.to_vec(), SYSEX_SHIFT_DOWN.to_vec()]);
        assert!(t.midi_mode);
        r[sh.byte] &= !sh.mask;
        r[ch.byte] &= !ch.mask;
        assert_eq!(t.input(&r), vec![SYSEX_SHIFT_UP.to_vec()]);
    }

    #[test]
    fn encoder_relative() {
        let mut t = started();
        let mut r = report();
        r[ENCODER_BYTE] = 1;
        assert_eq!(t.input(&r), vec![vec![0xB0, 7, 1]]);
        r[ENCODER_BYTE] = 0;
        assert_eq!(t.input(&r), vec![vec![0xB0, 7, 127]]);
        r[ENCODER_BYTE] = 15; // wrap backwards
        assert_eq!(t.input(&r), vec![vec![0xB0, 7, 127]]);
        r[ENCODER_BYTE] = 0; // wrap forwards
        assert_eq!(t.input(&r), vec![vec![0xB0, 7, 1]]);
    }

    #[test]
    fn knob_relative_with_wrap() {
        let mut t = Translator::new(Config { knob_divider: 8, ..Config::default() });
        t.input(&report());
        let mut r = report();
        let set = |r: &mut Vec<u8>, v: u16| r[KNOBS_OFFSET..KNOBS_OFFSET + 2].copy_from_slice(&v.to_le_bytes());
        set(&mut r, 16);
        assert_eq!(t.input(&r), vec![vec![0xB0, 70, 2]]);
        set(&mut r, 20); // +4: below the divider, accumulated
        assert!(t.input(&r).is_empty());
        set(&mut r, 24);
        assert_eq!(t.input(&r), vec![vec![0xB0, 70, 1]]);
        set(&mut r, 992); // -32 through the wrap
        assert_eq!(t.input(&r), vec![vec![0xB0, 70, 0x7c]]);
    }

    #[test]
    fn strip() {
        let mut t = started();
        let mut r = report();
        r[STRIP_OFFSET..STRIP_OFFSET + 2].copy_from_slice(&1023u16.to_le_bytes());
        assert_eq!(t.input(&r), vec![vec![0xB0, 2, 127], vec![0xB0, 1, 127]]);
        assert_eq!(t.input(&report()), vec![vec![0xB0, 2, 0]]);
    }

    #[test]
    fn pads_events() {
        let mut t = started();
        let mut p = vec![0u8; 64];
        p[0] = REPORT_PADS;
        // pad idx 12 (bottom left): pressure, note on (velocity 4095), pressure
        p[1..10].copy_from_slice(&[12, 0x40, 0x20, 12, 0x1f, 0xff, 12, 0x48, 0x00]);
        assert_eq!(
            t.input(&p),
            vec![vec![0x90, 60, 127], vec![0xA0, 60, 127]]
        );
        let mut p = vec![0u8; 64];
        p[0] = REPORT_PADS;
        p[1..4].copy_from_slice(&[12, 0x30, 0x00]);
        assert_eq!(t.input(&p), vec![vec![0x80, 60, 0]]);
    }

    /// Knob 8 untouched, values measured on the user's MK3: no MIDI at all.
    #[test]
    fn knob_jitter_is_silent() {
        let mut t = Translator::new(Config { knob_divider: 8, ..Config::default() });
        let mut r = report();
        let o = KNOBS_OFFSET + 14;
        r[o..o + 2].copy_from_slice(&680u16.to_le_bytes());
        t.init(&r); // initial state, as read with GET_REPORT at startup
        for v in [682u16, 678, 681, 677, 680, 677, 682, 679, 682, 678, 681, 677, 680, 681, 677, 683, 677] {
            r[o..o + 2].copy_from_slice(&v.to_le_bytes());
            let out = t.input(&r);
            assert!(out.iter().all(|m| m[1] != 77), "jitter produced {out:?} at {v}");
        }
        // a real turn still works, and jitter right after it doesn't add steps
        for v in [690u16, 700, 712] {
            r[o..o + 2].copy_from_slice(&v.to_le_bytes());
            t.input(&r);
        }
        for v in [708u16, 713, 709, 714, 710] {
            r[o..o + 2].copy_from_slice(&v.to_le_bytes());
            assert!(t.input(&r).is_empty(), "jitter after a turn at {v}");
        }
    }

    #[test]
    fn knob_fine_with_shift() {
        let mut t = Translator::new(Config { knob_divider: 14, ..Config::default() });
        t.input(&report());
        let mut r = report();
        let set = |r: &mut Vec<u8>, v: u16| r[KNOBS_OFFSET..KNOBS_OFFSET + 2].copy_from_slice(&v.to_le_bytes());
        set(&mut r, 28);
        assert_eq!(t.input(&r), vec![vec![0xB0, 70, 2]]); // 28 units = 2 steps
        let sh = btn("Shift");
        r[sh.byte] |= sh.mask;
        t.input(&r);
        set(&mut r, 56); // same 28 units with SHIFT: below 56 needed for one fine step
        assert!(t.input(&r).is_empty());
        set(&mut r, 84);
        assert_eq!(t.input(&r), vec![vec![0xB0, 70, 1]]);
    }

    #[test]
    fn values_row_sysex() {
        let mut t = started();
        let mut s = vec![0xF0, 0x00, 0x00, 0x66, 0x17, 0x13, 1];
        s.extend_from_slice(b"0.0 dB|-6 dB");
        s.push(0xF7);
        t.midi(&s);
        let row = DISPLAY_CHARS + VALUES_ROW;
        assert_eq!(&t.display[row..row + 12], b"0.0 dB|-6 dB");
        assert_eq!(t.display[row + 12], b' ');
    }

    fn pad(entries: &[[u8; 3]]) -> Vec<u8> {
        let mut p = vec![0u8; 64];
        p[0] = REPORT_PADS;
        for (i, e) in entries.iter().enumerate() {
            p[1 + 3 * i..4 + 3 * i].copy_from_slice(e);
        }
        p
    }

    /// Pad 14's soft hit from the user's dump: the firmware called it a light touch
    /// (event 0, pressure up to 84, event 2). It now plays a note.
    #[test]
    fn light_touch_plays_a_note() {
        let mut t = started();
        let mut out = vec![];
        for e in [[1, 0x00, 0x0f], [1, 0x40, 0x30], [1, 0x40, 0x54], [1, 0x40, 0x00], [1, 0x20, 0x00]] {
            out.extend(t.input(&pad(&[e])));
        }
        let v = Config::default().curve(84);
        assert_eq!(out, vec![vec![0x90, 73, v], vec![0x80, 73, 0]]);
        assert!(v >= 15, "velocity {v}");
    }

    /// Real hits from the dump (raw 161..2441) get usable velocities, not 5..20.
    #[test]
    fn velocity_curve() {
        let c = Config::default();
        assert!(c.curve(161) >= 25);
        assert!(c.curve(342) >= 40);
        assert!(c.curve(633) >= 55);
        assert_eq!(c.curve(2441), 127);
        assert_eq!(c.curve(0), 0);
        let lin = Config { pad_curve: 1.0, pad_vel_max: 4095, ..Config::default() };
        assert_eq!(lin.curve(4095), 127);
        assert_eq!(lin.curve(2048), 64);
    }

    #[test]
    fn pads_threshold() {
        let mut t = Translator::new(Config { pad_mode: PadMode::Threshold, ..Config::default() });
        t.input(&report());
        let mut p = vec![0u8; 64];
        p[0] = REPORT_PADS;
        p[1..7].copy_from_slice(&[3, 0x0f, 0xff, 3, 0x00, 0x10]);
        assert_eq!(t.input(&p), vec![vec![0x90, 75, 127], vec![0x80, 75, 0]]);
    }

    #[test]
    fn leds_from_midi() {
        let mut t = started();
        t.midi(&[0xB0, 57, 127]); // Play
        t.midi(&[0xB0, 100, 21]); // Group A colour
        t.midi(&[0xB0, 30, 127]); // Enc up arrow
        t.midi(&[0x90, 60, 45]); // Pad1
        t.midi(&[0xB0, 1, 127]); // strip full
        let (b, p) = t.leds();
        assert_eq!(b[41], 0x7e);
        assert_eq!(b[29], 21);
        assert_eq!(b[58], 70);
        assert_eq!(b[43], 0x7c); // Stop backlight
        assert_eq!(p[PAD_LEDS_OFFSET + 12], 45);
        assert!(p[..STRIP_LEDS].iter().all(|&v| v == 70));
        t.midi(&[0xB0, 57, 0]);
        t.midi(&[0x80, 60, 0]);
        let (b, p) = t.leds();
        assert_eq!(b[41], 0x7c);
        assert_eq!(p[PAD_LEDS_OFFSET + 12], 0);
    }

    #[test]
    fn display_sysex_split() {
        let mut t = started();
        let mut s = vec![0xF0, 0x00, 0x00, 0x66, 0x17, 0x12, 28];
        s.extend_from_slice(b"Hello");
        t.midi(&s);
        assert_eq!(&t.display[28..33], b"     ");
        t.midi(&[b'!', 0xF7]);
        assert_eq!(&t.display[28..34], b"Hello!");
    }
}
