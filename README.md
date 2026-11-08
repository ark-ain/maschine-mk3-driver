# maschine-mk3: Linux driver for the Maschine MK3, for Bitwig Studio

This project does on Linux what Native Instruments' software (NIHardwareAgent and Controller
Editor in **MIDI mode**) does on Windows and macOS, so that the **Maschine MK3** (17cc:1600) can be
used with **Bitwig Studio**. It has two parts:

- **the driver** (Rust, this folder): reads the Maschine over USB and presents it to Bitwig as a
  MIDI controller, drives the LEDs and draws both displays;
- **the "Maschine Mk3 (Linux)" Bitwig extension** (`bitwig-extension/`): Bitwig's own Maschine Mk3
  extension (open source, MIT), which Bitwig 6 refuses to start on Linux, unlocked and extended.
  Its buttons are described in `bitwig-extension/README.md`.

Features: pads (velocity, aftertouch, colours), every button including SHIFT, 4D encoder, 8 knobs
(fine adjustment with SHIFT), coloured group buttons A–H, touch strip, footswitch, and both
displays (parameter names and their values). **SHIFT + CHANNEL** turns MIDI mode off and on.

The driver runs in user space, with no kernel module, and only depends on `alsa-lib` and `libusb`.

## Status

Tested on a Maschine MK3 with Fedora 44 and Bitwig 6.1.4:

| Part | Source | Status |
|---|---|---|
| MIDI messages Bitwig expects | Bitwig extension source (`bitwig-extensions`) | tested |
| Buttons, knobs, encoder, strip (HID reports) | OpenAV **Ctlra** reverse engineering | tested |
| Pads: note-on/off/pressure events, order | jimpo's analysis of the Mikro MK3 | tested (dump) |
| LEDs and displays | Ctlra | tested |
| Touch strip range (0–1023) | Ctlra | not verified |

Known limitations:
- **Browser**: since Bitwig 5, the API no longer controls the content type, location, file type,
  device type or device filters; only category, tag and creator respond.
- **Modulators**: Bitwig has no action to show them; MOD opens the device's side panel instead.
- **Project scale**: not exposed by the API; chord mode follows the scale of KEYBOARD mode.
- The MK3's built-in **audio interface** (headphones, outputs) is **not** handled here: it depends
  on the kernel's USB audio driver (`snd-usb-audio`).

## 1. Install (Fedora)

```bash
sudo dnf install rust cargo alsa-lib-devel libusb1-devel
```

```bash
cd maschine-mk3-driver && ./install.sh
```

This single script installs everything:
- the driver in `/usr/local/bin/maschine-mk3`
- a udev rule that gives your user access to the device and starts the driver when it is plugged in
- a systemd user service, `maschine-mk3.service` (restarted if it was already running)
- the `snd-virmidi` configuration: a virtual MIDI port ("Virtual Raw MIDI/10" in Bitwig)
- the Bitwig extension in `~/Bitwig Studio/Extensions/MaschineMk3Linux.bwextension`
  (already compiled; `./install.sh --rebuild-extension` recompiles it, with `maven` and a JDK 21)

After the first install, **unplug and replug** the Maschine.

> If `snd-virmidi` was already loaded with other options: `sudo modprobe -r snd-virmidi`
> then `sudo modprobe snd-virmidi`, or reboot.

To uninstall everything:

```bash
./uninstall.sh
```

## 2. Set up Bitwig

1. Restart Bitwig.
2. **Settings → Controllers → Add Controller → Native Instruments → Maschine Mk3 (Linux)**.
3. Choose **Virtual Raw MIDI/10** as both the MIDI input **and** output.

Don't use Bitwig's built-in "Maschine Mk3" extension: on Linux it stops with "Maschine is not
supported on Linux". Bitwig remembers this setup: from then on, just plug the Maschine in, even
with Bitwig already open.

## 3. Using the Maschine

With the "Maschine Mk3 (Linux)" extension. "Hold X + Y" means keep X pressed and press Y.
Pad 1 is bottom left, pad 16 top right.

### Transport

| Control | Action |
|---|---|
| PLAY | play / stop |
| SHIFT + PLAY | restart from the start |
| STOP | stop |
| SHIFT + STOP | stop all clips |
| REC | record (arranger or launcher, see Focus below) |
| SHIFT + REC | launcher overdub on/off |
| RESTART | restart from the start |
| SHIFT + RESTART | arranger loop on/off |
| TAP | tap tempo |
| SHIFT + TAP | metronome on/off |
| FOLLOW | follow the playhead |
| SHIFT + FOLLOW | show/hide the clip launcher |
| AUTO | show/hide automation lanes of the selected tracks |
| SHIFT + AUTO | automation write on/off |
| FILE | save the project |
| SHIFT + FILE | open a project |

Focus: in the controller's settings, "Recording/Automation" chooses whether REC and SHIFT + AUTO
act on the arranger or on the clip launcher.

### Pads and pad modes

| Control | Action |
|---|---|
| PATTERN | pads launch clips (4 tracks × 4 scenes) |
| SCENE | pads launch scenes |
| PAD MODE | pads play the drum machine's pads, with their colours |
| KEYBOARD | pads play notes in a scale; knobs: scale, octave, semitone, fixed velocity |
| CHORDS | chord mode on/off: each pad plays a triad in the KEYBOARD scale |
| STEP | step sequencer: pads are the steps; knobs: note, velocity, loop length, grid, note length |
| FIXED VEL | fixed velocity on/off |
| NOTE REPEAT (held) | arpeggiate the notes you play while held |
| SHIFT + NOTE REPEAT | latched arpeggiator on/off; knobs: octaves, mode, rate, gate |

Hold one of these and press a pad (or a group button for tracks):

| Hold | + pad / group |
|---|---|
| SELECT | select without playing |
| SOLO | solo |
| MUTE | mute |
| ERASE | erase (drum pad notes, clip, scene, track) |
| DUPLICATE | duplicate (clip, scene, track) |

A short tap on SELECT selects all / nothing in the focused panel; a short tap on DUPLICATE
duplicates the selection.

SHIFT + pads: 1 Undo, 2 Redo, 3 Step Undo, 4 Step Redo, 5 Quantize, 6 Quantize 50 %,
7 Nudge ←, 8 Nudge →, 9 Clear, 10 Clear Auto, 11 Copy, 12 Paste, 13 Semitone −, 14 Semitone +,
15 Octave −, 16 Octave +.

### Tracks (group buttons A–H)

| Control | Action |
|---|---|
| A–H | select a track (the button after the last track creates a new instrument track) |
| SHIFT + A–H | jump between banks of 8 tracks (A = 1–8, B = 9–16…) |
| SAMPLING | new audio track after the selected one, armed |
| SHIFT + SAMPLING | new effect track |
| LOCK | pin/unpin the selected track and device |

### Displays, knobs and the buttons above the displays

| Control | Action |
|---|---|
| MIXER | knobs = volume of 8 tracks; ◀ ▶ switch volume / pan / sends. Also shows/hides Bitwig's mixer panel |
| SHIFT + MIXER | Bitwig's MIX view |
| PLUG-IN | knobs = the selected device's 8 remote controls; ◀ ▶ change page. Buttons above: 1 bypass, 3 / 4 previous / next device |
| SHIFT + PLUG-IN | open/close the plug-in's window |
| MACRO (held) | finer knobs in device mode |
| knobs | turn = change; touch = show the value on the display |
| SHIFT + knob | 4× finer adjustment |

Each display shows the names of its 4 knobs and their values underneath.

### Browser

| Control | Action |
|---|---|
| BROWSER | open the browser to add a device after the selected one (close it again with BROWSER) |
| SHIFT + BROWSER | Bitwig's browser panel on the right |
| knobs 1–7 | filters: location, file type, category, tag, creator, device type, device (since Bitwig 5 only category, tag and creator respond) |
| knob 8 / big encoder | scroll the results |
| push the big encoder | load the selected item |
| button 4 above the displays | audition on/off |

### Big 4D encoder

| Control | Action |
|---|---|
| turn | depends on the mode (results in the browser, VOLUME / SWING / TEMPO below…) |
| push | confirm (load in the browser) |
| tilt ◀ ▶ ▲ ▼ | move through tracks and scenes |
| VOLUME, then turn | main volume (SHIFT: finer) |
| SWING, then turn | groove shuffle (SHIFT: accent) |
| TEMPO, then turn | tempo (SHIFT: finer) |

### Bitwig panels and views

| Control | Action |
|---|---|
| ARRANGER | switch between the ARRANGE and MIX views |
| SHIFT + ARRANGER | note editor |
| NOTES | EDIT view |
| EVENTS | detail editor |
| SHIFT + EVENTS | EDIT view |
| VARIATION | device panel |
| CHANNEL | device panel |
| MOD | the device's side panel (remote controls, modulators below) |
| SETTINGS | inspector |
| SHIFT + SETTINGS | project panel |

### Touch strip and footswitch

| Control | Action |
|---|---|
| PITCH | strip = pitch bend (springs back to the centre); press again to turn off |
| PERFORM | strip = the selected device's 1st remote control; press again to turn off |
| strip LEDs | position of the playing clip |
| footswitch | sends CC 64 (not used by the extension yet) |

### Driver

| Control | Action |
|---|---|
| SHIFT + CHANNEL | MIDI mode off/on (the Maschine goes dark and stops sending) |

## 4. Check the hardware (5 minutes, once)

Stop the service first, so the driver releases the device:

```bash
systemctl --user stop maschine-mk3
```

**a) Buttons, knobs and pads**

```bash
maschine-mk3 dump
```

Press a few buttons: each one should print its name (`Play PRESSED`, `Group A PRESSED`…). Turn
the knobs and the encoder, touch the strip, then hit pads 1 (bottom left), 4, 13 and 16. For each
pad, check:
- that the note shown is 60 for pad 1, 63 for pad 4, 72 for pad 13 and 75 for pad 16
- that you see `event=1` when you hit it and `event=3` when you release it

If you never see `event=1` or `event=3`, run the driver with `--pad-mode threshold`.

**b) LEDs**

```bash
maschine-mk3 led-test
```

The test lights each LED in turn and prints the expected button. It then lights pads 1 to 16 in
order, cycles through the 17 colours and the strip LEDs. To try another brightness, for example
`0x40`:

```bash
maschine-mk3 led-test 0x40
```

**c) Displays**

```bash
maschine-mk3 screen-test
```

Each display should show colour bars in the order red, green, blue, yellow, cyan, magenta, white
and grey, then "SCREEN 0" or "SCREEN 1".

Then start the service again:

```bash
systemctl --user start maschine-mk3
```

## Running the driver by hand / options

```bash
maschine-mk3 --help
```

| Option | Purpose |
|---|---|
| `--port NAME` or `client:port` | sequencer port to use (default: the single `VirMIDI N-0`) |
| `--no-screens` | don't use the displays |
| `--knob-divider N` | sensitivity of the 8 knobs (default 13; smaller = faster; 4× finer with SHIFT) |
| `--knob-hysteresis N` | extra movement needed to reverse a knob, against sensor jitter (default 6) |
| `--pad-mode events\|threshold` | how pad hits are detected |
| `--pad-threshold N` | threshold for `threshold` mode (0–4095, default 128) |
| `--pad-vel-max N` | raw pad value that gives velocity 127 (default 2048; smaller = louder) |
| `--pad-curve X` | velocity curve: 1 = linear, < 1 = soft hits louder (default 0.6) |
| `--pad-touch-trigger N` | light touches reaching this pressure play a note (default 70, 0 = off) |
| `--led-dim N` / `--led-on N` | button brightness when idle and when lit |

To keep an option with the service, add it to the `ExecStart=` line of
`/etc/systemd/user/maschine-mk3.service`, then run `systemctl --user daemon-reload`.

Ctrl-C turns the LEDs and displays off. Exit status: 0 on SIGINT/SIGTERM, 3 when the device is not
plugged in, 1 for other errors (including unplugging it while running).

## Troubleshooting

- **`Maschine MK3 (17cc:1600) not found`**: check with `lsusb | grep 17cc` and
  `grep -l 1600 /sys/class/hidraw/*/device/uevent`.
- **Permission denied on `/dev/hidrawN`**: the udev rule is not active. Run
  `sudo udevadm control --reload`, then unplug and replug.
- **`displays unavailable`**: USB access to the displays is denied (udev rule) or the interface is
  busy. Pads and buttons still work.
- **`no VirMIDI port found`**: run `sudo modprobe snd-virmidi`.
- **Nothing reaches Bitwig**: `aconnect -l` should show the "Maschine MK3" client connected to
  "VirMIDI 10-0", both ways.
- **Logs**: `journalctl --user -u maschine-mk3 -f`

## How it works

```
Maschine MK3 ──hidraw (interface 4)──► maschine-mk3 ◄──ALSA sequencer──► VirMIDI 10-0 ◄──rawmidi──► Bitwig
             ──libusb (interface 5)──► displays                                   ("Maschine Mk3 (Linux)" extension)
```

| File | Role |
|---|---|
| `src/protocol.rs` | **the table**: button bits, LED numbers, CC and note numbers, sysex |
| `src/translate.rs` | HID ↔ MIDI translation, no I/O (this is what the tests cover) |
| `src/hidraw.rs` | finds and opens the hidraw node, reads report sizes from the HID descriptor |
| `src/seq.rs` | ALSA sequencer client connected to the VirMIDI port |
| `src/screen.rs` | draws Bitwig's text on the 480×272 displays, sends it over USB bulk from its own thread |
| `src/main.rs` | `poll()` loop (pads, buttons, MIDI), LED thread, `dump`, `led-test` and `screen-test` tools |

MIDI mapping (channel 1), the same as NI's Bitwig template:
- pads: notes 60–75 (pad 1 = 60, bottom left); the LED colour is the velocity of the note received
- buttons: CC 127 on press and 0 on release; the same CC drives the LED.
  Play 57, Rec 58, Stop 59, Restart 53, Erase 54, Tap 60, Follow 56, groups A–H 100–107,
  buttons above the displays 22–29, knobs 70–77 (relative), knob touch 10–17, encoder 7, push 8,
  touch 9, directions 30–33, page ←/→ 110/111, etc. The full list is in `src/protocol.rs`.
- SHIFT: sysex `F0 00 21 09 16 00 4D 50 00 01 4D 01/00 F7`
- display text: sysex `F0 00 00 66 17 12 <offset> <28 chars> F7`.
  Left display = offsets 0 (top line) and 56 (bottom line); right display = 28 and 84.
- knob values under the names (Linux extension only): sysex
  `F0 00 00 66 17 13 <display 0/1> <4 values separated by '|'> F7`.

```bash
cargo test
```

## Credits

- [jimpo/mikro-mk3-driver](https://github.com/jimpo/mikro-mk3-driver): architecture
  (hidraw, VirMIDI, systemd) and analysis of NI's MIDI mode on the Mikro MK3
- [OpenAV Ctlra](https://github.com/openAVproductions/openAV-Ctlra): the MK3's HID and display protocol
- [bitwig/bitwig-extensions](https://github.com/bitwig/bitwig-extensions): what the Bitwig extension expects
- [git-moss/DrivenByMoss](https://github.com/git-moss/DrivenByMoss): browser handling in the extension
- [wrl/maschine.rs](https://github.com/wrl/maschine.rs)

License: driver GPL-3.0-or-later. The extension is derived from Bitwig's extensions (MIT).
