# "Maschine Mk3 (Linux)" Bitwig extension

Bitwig 6 refuses to start its built-in "Maschine Mk3" extension on Linux
("Maschine is not supported on Linux since there is no Native Instruments DAW Integration Host").
This folder contains **the open source code of that same extension** (`bitwig/bitwig-extensions`,
MIT license), without that check, with its own name and ID, plus additions.

## Install
It is installed by `../install.sh`, together with the driver. To install it alone:
```bash
./build.sh
```
`MaschineMk3Linux.bwextension` is already compiled (checked against Bitwig APIs 18 and 25).
To recompile it from the sources: `./build.sh --rebuild` (needs `maven` and `java-21-openjdk-devel`).

In Bitwig: Settings → Controllers → Add Controller → Native Instruments → **Maschine Mk3 (Linux)**,
with **Virtual Raw MIDI/10** as input and output.

## Additions to Bitwig's extension
| Button | Action |
|---|---|
| EVENTS | shows/hides the detail editor (piano roll / audio) |
| SHIFT + EVENTS | Bitwig's EDIT view |
| NOTES | Bitwig's EDIT view |
| SETTINGS | shows/hides the inspector |
| SHIFT + SETTINGS | Project panel (`focus_or_toggle_song_panel`) |
| AUTO | shows/hides the automation lanes of the selected tracks (`toggle_automation_shown_for_selected_tracks`) |
| SHIFT + AUTO | automation write (AUTO's original function) |
| VARIATION | shows/hides the track's device panel |
| MOD | the device's side panel (remote controls, modulators below them): Bitwig has no "show modulators" action |
| MIXER | Maschine mixer mode (knobs/displays) + shows/hides the small mixer at the bottom |
| SHIFT + MIXER | Bitwig's MIX view |
| BROWSER | the Maschine browser (unchanged, see below) |
| SHIFT + BROWSER | shows/hides Bitwig's Browser panel on the right (`focus_or_toggle_browser_panel`) |
| DUPLICATE (short tap) | duplicates the selection (Ctrl+D) |
| DUPLICATE held + pad/group | duplicates that pad / track (unchanged) |
| SHIFT + DUPLICATE | duplicate as alias (`Duplicate Reference`) |
| SELECT (short tap) | select all / select none in the focused panel |
| SELECT held + pad | selects the pad without playing it (unchanged) |
| SAMPLING | creates an audio track after the selected one, and arms it |
| SHIFT + SAMPLING | creates an effect track |
| NOTE REPEAT (held) | arpeggiates while held (unchanged) |
| SHIFT + NOTE REPEAT | latched arpeggiator on/off; the knobs set octaves, mode, rate, gate |
| CHANNEL | shows/hides the device panel (SHIFT + CHANNEL = MIDI mode, handled by the driver) |
| LOCK | pins/unpins the selected device and track |
| PITCH | touch strip = pitch bend (springs back to the centre on release); press again to turn off |
| PERFORM | touch strip = 1st parameter (remote control 1) of the selected device; press again to turn off |
| CHORDS | chord mode: each pad plays a triad in the KEYBOARD mode scale |
| SHIFT + ARRANGER | shows/hides the note editor |
| SHIFT + PLUG-IN | opens/closes the selected plug-in's window |

SHIFT + pads (pad 1 = bottom left): 1 Undo, 2 Redo, 3 Step Undo (= Undo), 4 Step Redo (= Redo),
5 Quantize, 6 Quantize 50 %, 7 Nudge ←, 8 Nudge →, 9 Clear, 10 Clear Auto (restores automation),
11 Copy, 12 Paste, 13 Semitone −, 14 Semitone +, 15 Octave −, 16 Octave +.

Displays: each parameter's name stays visible, with its value always shown underneath, above the
matching knob (device and mixer modes).

Knobs: slower in device mode (×1.5 instead of ×4; with MACRO held: ×0.5).

Browser (BROWSER), rewritten after DrivenByMoss: knobs 1–7 = Bitwig's filter columns (location,
file type, category, tag, creator, device type, device), knob 8 = results, with each column's name
on the display and the current choice underneath; 2 knob notches per item. Since Bitwig 5 the API
no longer controls the content type, and only category, tag and creator respond (also noted in
DrivenByMoss). The big encoder also scrolls the results, pushing it loads the item; the 4th button
above the displays toggles audition.

Chords and pitch bend go through a second note input, "Chords+Strip", on MIDI channel 16. A track
whose input is "All ins" (the default) receives them.

At startup the extension lists every Bitwig action (id and name) in the controller console and in
`~/maschine-mk3-bitwig-actions.txt`, to find the ids of actions the API doesn't expose directly.

## Changes to Bitwig's code
- package `maschine` renamed `maschinelinux`, name "Maschine Mk3 (Linux)", new UUID
- Maschine+ extension removed
- EVENTS is no longer the clip-launch modifier (replaced by the detail editor)
- VARIATION no longer picks colours (replaced by the device panel)
- browser layer rewritten (`display/BrowserLayer.java`), knob increments counted directly
  (`display/DisplayLayer.java`)
- additions above: `MaschineExtension.java` (`setUpLinuxButtons`, `handleShiftAction`),
  `modes/KeyboardMode.java` (chord mode), `display/KnobParamLayer.java` (values line),
  `CcAssignment.java` (NOTES, LOCK, PITCH, PERFORM, MOD)
