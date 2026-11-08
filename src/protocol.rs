//! Maschine MK3 (17cc:1600): USB/HID layout and the MIDI numbers that Bitwig's
//! "Maschine Mk3" extension expects (the ones NI's Controller Editor template sends).
//!
//! HID side: from the reverse engineering in OpenAV Ctlra (`ni_maschine_mk3.c`).
//! MIDI side: from Bitwig's extension source (`nativeinstruments/maschine/CcAssignment.java`,
//! `MaschineExtension.java`, `buttons/*.java`, `MaschineMode.java`).
//!
//! Byte offsets in this file include the report ID (byte 0 = report ID), like hidraw gives them.

pub const VID: u16 = 0x17cc;
pub const PID: u16 = 0x1600;

/// USB interface carrying the two displays (bulk OUT endpoint 0x04).
pub const SCREEN_INTERFACE: u8 = 5;
pub const SCREEN_ENDPOINT: u8 = 0x04;
pub const SCREEN_W: usize = 480;
pub const SCREEN_H: usize = 272;

/// Input report: buttons, knobs, main encoder, touch strip, pedal.
pub const REPORT_BUTTONS: u8 = 0x01;
/// Input report: pads (list of `idx, event<<4 | value[11:8], value[7:0]` triples).
pub const REPORT_PADS: u8 = 0x02;
/// Output report: button LEDs (62 bytes, index = `led` below).
pub const REPORT_LEDS_BUTTONS: u8 = 0x80;
/// Output report: touch strip LEDs (25) then pad LEDs (16).
pub const REPORT_LEDS_PADS: u8 = 0x81;

pub const BUTTON_LEDS: usize = 62;
pub const STRIP_LEDS: usize = 25;
pub const PAD_LEDS_OFFSET: usize = 25;
pub const PAD_LED_BYTES: usize = 41;

/// Main encoder position: low nibble of this byte (wraps at 16).
pub const ENCODER_BYTE: usize = 11;
/// 8 knobs under the screens: u16 little-endian at 12 + 2*i, 0..999 (wraps).
pub const KNOBS_OFFSET: usize = 12;
pub const KNOB_RANGE: i32 = 1000;
/// Touch strip: u16 little-endian, 0 = not touched.
pub const STRIP_OFFSET: usize = 30;
pub const STRIP_MAX: u32 = 1023;
/// Footswitch (pedal): byte 3, bit 0x40.
pub const PEDAL_BYTE: usize = 3;
pub const PEDAL_MASK: u8 = 0x40;

/// MIDI channel 1 (0 on the wire) for everything, like the NI template.
pub const CC: u8 = 0xB0;
pub const NOTE_ON: u8 = 0x90;
pub const NOTE_OFF: u8 = 0x80;
pub const POLY_AT: u8 = 0xA0;

pub const CC_ENCODER_TURN: u8 = 7;
/// Knob i turns send CC 70+i, relative two's complement.
pub const CC_KNOB_BASE: u8 = 70;
pub const CC_STRIP: u8 = 1;
pub const CC_STRIP_TOUCH: u8 = 2;
pub const CC_PEDAL: u8 = 64;
/// Pad notes 60..75: Pad1 (bottom left) = 60.
pub const PAD_NOTE_BASE: u8 = 60;

/// What a button does on the MIDI side.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// CC 127 on press, 0 on release; the same CC drives the LED.
    Cc(u8),
    /// SHIFT: sends NI's shift sysex, LED lit while held.
    Shift,
}

/// How the button's LED reacts to its CC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Led {
    /// No LED.
    None,
    /// Single colour: CC 0 = backlight, CC > 0 = lit.
    Mono(usize),
    /// RGB, Bitwig sends the NI colour index (colour*4 + brightness) as CC value.
    Color(usize),
    /// RGB, but Bitwig sends on/off (0/127): shown white.
    ColorOnOff(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Button {
    pub name: &'static str,
    pub byte: usize,
    pub mask: u8,
    pub action: Action,
    pub led: Led,
}

const fn b(name: &'static str, byte: usize, mask: u8, cc: u8, led: Led) -> Button {
    Button { name, byte, mask, action: Action::Cc(cc), led }
}

use Led::{Color, ColorOnOff, Mono};

/// Every button. CC numbers: Bitwig's `CcAssignment` for the MK3. Buttons Bitwig doesn't use
/// (Notes, Pitch, Mod, Perform, Lock) get free CC numbers so they can still be MIDI-mapped.
pub const BUTTONS: &[Button] = &[
    // 4D encoder
    b("Enc. Push", 1, 0x01, 8, Led::None),
    b("Enc. Right", 1, 0x08, 31, ColorOnOff(60)),
    b("Enc. Up", 1, 0x04, 30, ColorOnOff(58)),
    b("Enc. Left", 1, 0x20, 33, ColorOnOff(59)),
    b("Enc. Down", 1, 0x10, 32, ColorOnOff(61)),
    Button { name: "Shift", byte: 1, mask: 0x40, action: Action::Shift, led: Mono(44) },
    b("Top 8", 1, 0x80, 29, Mono(19)),
    // Groups A..H
    b("Group A", 2, 0x01, 100, Color(29)),
    b("Group B", 2, 0x02, 101, Color(30)),
    b("Group C", 2, 0x04, 102, Color(31)),
    b("Group D", 2, 0x08, 103, Color(32)),
    b("Group E", 2, 0x10, 104, Color(33)),
    b("Group F", 2, 0x20, 105, Color(34)),
    b("Group G", 2, 0x40, 106, Color(35)),
    b("Group H", 2, 0x80, 107, Color(36)),
    // Notes, Volume, Swing, Tempo, Note Repeat, Lock
    b("Notes", 3, 0x01, 52, Mono(28)),
    b("Volume", 3, 0x02, 44, Mono(20)),
    b("Swing", 3, 0x04, 45, Mono(21)),
    b("Tempo", 3, 0x08, 47, Mono(23)),
    b("Note Repeat", 3, 0x10, 46, Mono(22)),
    b("Lock", 3, 0x20, 48, Mono(24)),
    // Pad Mode, Keyboard, Chords, Step, Fixed Vel, Scene, Pattern, Events
    b("Pad Mode", 4, 0x01, 80, Mono(46)),
    b("Keyboard", 4, 0x02, 82, Mono(47)),
    b("Chords", 4, 0x04, 84, Mono(48)),
    b("Step", 4, 0x08, 83, Mono(49)),
    b("Fixed Vel", 4, 0x10, 81, Mono(45)),
    b("Scene", 4, 0x20, 85, Mono(50)),
    b("Pattern", 4, 0x40, 86, Mono(51)),
    b("Events", 4, 0x80, 87, Mono(52)),
    // Variation, Duplicate, Select, Solo, Mute, Pitch, Mod
    b("Variation", 5, 0x02, 88, Mono(53)),
    b("Duplicate", 5, 0x04, 89, Mono(54)),
    b("Select", 5, 0x08, 90, Mono(55)),
    b("Solo", 5, 0x10, 91, Mono(56)),
    b("Mute", 5, 0x20, 92, Mono(57)),
    b("Pitch", 5, 0x40, 49, Mono(25)),
    b("Mod", 5, 0x80, 50, Mono(26)),
    // Perform, Restart, Erase, Tap, Follow, Play, Rec, Stop
    b("Perform", 6, 0x01, 51, Mono(27)),
    b("Restart", 6, 0x02, 53, Mono(37)),
    b("Erase", 6, 0x04, 54, Mono(38)),
    b("Tap", 6, 0x08, 60, Mono(39)),
    b("Follow", 6, 0x10, 56, Mono(40)),
    b("Play", 6, 0x20, 57, Mono(41)),
    b("Rec", 6, 0x40, 58, Mono(42)),
    b("Stop", 6, 0x80, 59, Mono(43)),
    // Macro, Settings, Right arrow, Sampling, Mixer, Plug-in
    b("Macro", 7, 0x01, 43, Mono(11)),
    b("Settings", 7, 0x02, 41, Mono(9)),
    b("Page Right", 7, 0x04, 111, Mono(7)),
    b("Sampling", 7, 0x08, 39, ColorOnOff(5)),
    b("Mixer", 7, 0x10, 37, Mono(3)),
    b("Plug-in", 7, 0x20, 35, Mono(1)),
    // Channel (MIDI), Arranger, Browser, Left arrow, File (Save), Auto
    b("Channel", 8, 0x01, 34, Mono(0)),
    b("Arranger", 8, 0x02, 36, Mono(2)),
    b("Browser", 8, 0x04, 38, Mono(4)),
    b("Page Left", 8, 0x08, 110, Mono(6)),
    b("File", 8, 0x10, 40, Mono(8)),
    b("Auto", 8, 0x20, 42, Mono(10)),
    // The 8 buttons above the screens
    b("Top 1", 9, 0x01, 22, Mono(12)),
    b("Top 2", 9, 0x02, 23, Mono(13)),
    b("Top 3", 9, 0x04, 24, Mono(14)),
    b("Top 4", 9, 0x08, 25, Mono(15)),
    b("Top 5", 9, 0x10, 26, Mono(16)),
    b("Top 6", 9, 0x20, 27, Mono(17)),
    b("Top 7", 9, 0x40, 28, Mono(18)),
    // Touch sensors
    b("Enc. Touch", 9, 0x80, 9, Led::None),
    b("Knob 1 Touch", 10, 0x80, 10, Led::None),
    b("Knob 2 Touch", 10, 0x40, 11, Led::None),
    b("Knob 3 Touch", 10, 0x20, 12, Led::None),
    b("Knob 4 Touch", 10, 0x10, 13, Led::None),
    b("Knob 5 Touch", 10, 0x08, 14, Led::None),
    b("Knob 6 Touch", 10, 0x04, 15, Led::None),
    b("Knob 7 Touch", 10, 0x02, 16, Led::None),
    b("Knob 8 Touch", 10, 0x01, 17, Led::None),
];

/// The button that, with SHIFT, toggles MIDI mode (on the MK3 it's labelled "CHANNEL / MIDI").
pub const MIDI_MODE_TOGGLE: &str = "Channel";

/// NI sysex sent to the DAW. Device ID 16 00 = Maschine MK3 (`MaschineMode.MK3`).
pub const SYSEX_SHIFT_DOWN: [u8; 13] = [0xF0, 0x00, 0x21, 0x09, 0x16, 0x00, 0x4D, 0x50, 0x00, 0x01, 0x4D, 0x01, 0xF7];
pub const SYSEX_SHIFT_UP: [u8; 13] = [0xF0, 0x00, 0x21, 0x09, 0x16, 0x00, 0x4D, 0x50, 0x00, 0x01, 0x4D, 0x00, 0xF7];
/// "MIDI mode entered": Bitwig answers by re-sending all its LED state.
pub const SYSEX_RETURN_FROM_HOST: [u8; 13] =
    [0xF0, 0x00, 0x21, 0x09, 0x16, 0x00, 0x4D, 0x50, 0x00, 0x01, 0x46, 0x01, 0xF7];

/// Display text from Bitwig, Mackie-style: `F0 00 00 66 17 12 <offset> <chars…> F7`.
/// Bitwig sends 28 chars at offsets 0, 28, 56, 84 (4 cells: 2 per screen, 2 lines).
pub const SYSEX_DISPLAY_PREFIX: [u8; 4] = [0xF0, 0x00, 0x00, 0x66];
pub const DISPLAY_CHARS: usize = 112;
/// Linux port: a 3rd line per screen with the 4 knob values, `F0 00 00 66 17 13 <screen> <text> F7`,
/// fields separated by '|'. Stored after the 112 chars, `VALUES_ROW` bytes per screen.
pub const SYSEX_VALUES_CMD: u8 = 0x13;
pub const VALUES_ROW: usize = 56;
pub const DISPLAY_TOTAL: usize = DISPLAY_CHARS + 2 * VALUES_ROW;
pub const DISPLAY_CELL: usize = 28;

/// HID pad index (0 = top left, row by row) → MIDI note. Pad1 = bottom left = note 60.
pub fn pad_note(idx: usize) -> u8 {
    PAD_NOTE_BASE + ((3 - idx / 4) * 4 + idx % 4) as u8
}

/// MIDI note → HID pad index, if it's one of the 16 pad notes.
pub fn note_pad(note: u8) -> Option<usize> {
    let n = note.checked_sub(PAD_NOTE_BASE)? as usize;
    (n < 16).then(|| (3 - n / 4) * 4 + n % 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_mapping_roundtrip() {
        for i in 0..16 {
            assert_eq!(note_pad(pad_note(i)), Some(i));
        }
        assert_eq!(pad_note(12), 60); // bottom left
        assert_eq!(pad_note(3), 75); // top right
        assert_eq!(note_pad(59), None);
        assert_eq!(note_pad(76), None);
    }

    #[test]
    fn table_is_consistent() {
        let mut bits = std::collections::HashSet::new();
        let mut leds = std::collections::HashSet::new();
        let mut ccs = std::collections::HashSet::new();
        for b in BUTTONS {
            assert!(bits.insert((b.byte, b.mask)), "duplicate bit {}", b.name);
            assert!(b.byte >= 1 && b.byte <= 10);
            if let Action::Cc(cc) = b.action {
                assert!(ccs.insert(cc), "duplicate CC {} ({})", cc, b.name);
                assert!(cc < 128);
            }
            match b.led {
                Led::Mono(i) | Led::Color(i) | Led::ColorOnOff(i) => {
                    assert!(i < BUTTON_LEDS);
                    assert!(leds.insert(i), "duplicate LED {}", b.name);
                }
                Led::None => {}
            }
        }
        assert!(BUTTONS.iter().any(|b| b.name == MIDI_MODE_TOGGLE));
    }
}
