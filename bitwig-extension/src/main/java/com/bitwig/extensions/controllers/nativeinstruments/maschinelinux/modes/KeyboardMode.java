package com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes;

import com.bitwig.extension.controller.api.CursorTrack;
import com.bitwig.extension.controller.api.HardwareButton;
import com.bitwig.extension.controller.api.InternalHardwareLightState;
import com.bitwig.extension.controller.api.NoteInput;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.MaschineExtension;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.NIColorUtil;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.MaschineLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.RgbLed;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.buttons.PadButton;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.ScaleLayer;
import com.bitwig.extensions.framework.values.Scale;

import java.util.List;
import java.util.function.DoubleConsumer;

public class KeyboardMode extends BasicKeyPlayingMode {

   private static final List<Scale> scales = List.of(Scale.CHROMATIC, Scale.MAJOR, Scale.MINOR, Scale.PENTATONIC,
      Scale.PENTATONIC_MINOR, Scale.DORIAN, Scale.PHRYGIAN, Scale.LYDIAN, Scale.MIXOLYDIAN, Scale.LOCRIAN,
      Scale.DIMINISHED, Scale.MAJOR_BLUES, Scale.MINOR_BLUES, Scale.WHOLE_TONE, Scale.ARABIAN, Scale.EGYPTIAN,
      Scale.GYPSI, Scale.SPANISH);

   private static final RgbLed BASENOTE_COLOR = RgbLed.of(73);
   private static final RgbLed BASENOTE_COLOR_ON = RgbLed.of(75);

   private Scale currentScale = scales.get(0);
   private final boolean[] isBaseNote = new boolean[16];

   private int baseNote = 0;
   private int octaveOffset = 0;

   private int currentScaleIndex = 0;
   private int trackColor = 0;

   // Linux port: chord mode. Each pad plays a triad built on its scale degree.
   private boolean chordMode = false;
   private final MaschineLayer chordLayer;
   private final int[][] heldChords = new int[16][];
   
   public KeyboardMode(final MaschineExtension driver, //
                       final String name, //
                       final NoteFocusHandler noteFocusHandler, //
                       final VeloctiyHandler velocityHandler, //
                       final ScaleLayer associatedDisplay) {
      super(driver, name, noteFocusHandler, velocityHandler, associatedDisplay);

      associatedDisplay.setKeyboardLayer(this);
      final NoteInput noteInput = driver.getNoteInput();
      noteInput.setShouldConsumeEvents(false);
      noteInput.setKeyTranslationTable(noteTable);
      velocityHandler.assingTranslationTable(noteInput);
      final PadButton[] buttons = driver.getPadButtons();
      final CursorTrack cursorTrack = driver.getCursorTrack();
      cursorTrack.color().addValueObserver((r, g, b) -> trackColor = NIColorUtil.convertColorX(r, g, b));
      for (int i = 0; i < buttons.length; i++) {
         final int index = i;
         final PadButton button = buttons[i];
         bindLightState(() -> computeGridLedState(index), button);
         bindShift(button);
         selectLayer.bindPressed(button, () -> selectPad(index));
      }
      chordLayer = new MaschineLayer(driver, "chord-" + name);
      for (int i = 0; i < buttons.length; i++) {
         final int index = i;
         final HardwareButton hw = buttons[i].getHwButton();
         chordLayer.bindPressed(hw, (DoubleConsumer) velocity -> chordOn(index, velocity));
         chordLayer.bindReleased(hw, (Runnable) () -> chordOff(index));
      }
   }

   public boolean isChordMode() {
      return chordMode;
   }

   public void setChordMode(final boolean on) {
      if (on == chordMode) {
         return;
      }
      chordMode = on;
      if (isActive()) {
         if (on) {
            chordLayer.activate();
         } else {
            releaseAllChords();
            chordLayer.deactivate();
         }
         applyScale();
      }
   }

   /** Root, third and fifth of the pad's scale degree (major triad in the chromatic scale). */
   private int[] chordNotes(final int index) {
      final int startNote = 60 + octaveOffset * 12 + baseNote;
      final int[] intervals = currentScale.getIntervals();
      final int[] notes = new int[3];
      if (intervals.length >= 12) {
         final int root = startNote + index;
         notes[0] = root;
         notes[1] = root + 4;
         notes[2] = root + 7;
      } else {
         for (int k = 0; k < 3; k++) {
            final int degree = index + 2 * k;
            notes[k] = startNote + intervals[degree % intervals.length] + 12 * (degree / intervals.length);
         }
      }
      for (int k = 0; k < 3; k++) {
         notes[k] = Math.min(127, Math.max(0, notes[k]));
      }
      return notes;
   }

   private void chordOn(final int index, final double velocity) {
      chordOff(index);
      final int vel = Math.max(1, Math.min(127, (int) Math.round(velocity * 127)));
      final int[] notes = chordNotes(index);
      for (final int note : notes) {
         getDriver().getExtrasInput().sendRawMidiEvent(0x9F, note, vel);
      }
      heldChords[index] = notes;
   }

   private void chordOff(final int index) {
      final int[] notes = heldChords[index];
      if (notes == null) {
         return;
      }
      heldChords[index] = null;
      for (final int note : notes) {
         getDriver().getExtrasInput().sendRawMidiEvent(0x8F, note, 0);
      }
   }

   private void releaseAllChords() {
      for (int i = 0; i < 16; i++) {
         chordOff(i);
      }
   }

   @Override
   public void doActivate() {
      super.doActivate();
      if (chordMode) {
         chordLayer.activate();
      }
   }

   @Override
   public void doDeactivate() {
      releaseAllChords();
      chordLayer.deactivate();
      super.doDeactivate();
   }

   public Scale getCurrentScale() {
      return currentScale;
   }

   public int getNextNote(final int noteVal, final int amount) {
      return currentScale.getNextNote(noteVal, baseNote, amount);
   }

   private void selectPad(final int index) {
      noteFocusHandler.notifyNoteSelected(noteTable[index + PadButton.PAD_NOTE_OFFSET]);
   }

   public void setAltMode(final PadMode altMode) {
      final MaschineExtension driver = getDriver();
      driver.getPrimaryDevice().hasDrumPads().addValueObserver(hasDrumPads -> {
         if (isActive() && hasDrumPads) {
            driver.setMode(altMode);
            if (altMode.getAssociatedDisplay() != null //
               && driver.getCurrentDisplayMode().isPadRelatedMode()) {
               driver.setDisplayMode(altMode.getAssociatedDisplay());
            }
         }
      });
   }

   public void incScale(final int dir) {
      final int nxtIndex = currentScaleIndex + dir;
      if (nxtIndex >= 0 && nxtIndex < scales.size()) {
         currentScale = scales.get(nxtIndex);
         currentScaleIndex = nxtIndex;
         while (currentScale.highestNote(60 + octaveOffset * 12 + baseNote, 16) > 127) {
            octaveOffset--;
         }
         applyScale();
      }
   }

   public void incOctave(final int incval) {
      final int newOctave = octaveOffset + incval;
      final int startNote = 60 + newOctave * 12 + baseNote;
      if (startNote > 0 && currentScale.highestNote(startNote, 16) < 128) {
         octaveOffset = newOctave;
         applyScale();
      }
   }

   public void incSemi(final int incval) {
      int newSemi = baseNote + incval;
      int newOctave = octaveOffset;
      if (newSemi < 0) {
         newOctave--;
         newSemi = 12 + incval;
      }
      if (newSemi > 11) {
         newOctave++;
         newSemi -= 12;
      }

      final int startNote = 60 + newOctave * 12 + newSemi;
      if (startNote > 0 && currentScale.highestNote(startNote, 16) < 128) {
         baseNote = newSemi;
         octaveOffset = newOctave;
         applyScale();
      }
   }

   private InternalHardwareLightState computeGridLedState(final int index) {
      if (isBaseNote[index]) {
         return playing[index] ? BASENOTE_COLOR_ON : BASENOTE_COLOR;
      }
      return RgbLed.of(trackColor + (playing[index] ? 2 : 0)); // Fast color look up with value of
   }

   @Override
   void applyScale() {
      for (int i = 0; i < 128; i++) {
         noteToPad[i] = -1;
      }

      final int startNote = 60 + octaveOffset * 12 + baseNote;
      final int[] intervals = currentScale.getIntervals();
      for (int i = 0; i < 16; i++) {
         final int index = i % intervals.length;
         final int oct = i / intervals.length;
         final int note = Math.min(127, Math.max(0, startNote + intervals[index] + 12 * oct));
         noteTable[i + PadButton.PAD_NOTE_OFFSET] = note;
         noteToPad[note] = i;
         isBaseNote[i] = index == 0;
         if (chordMode) {
            // pads don't play single notes: the chord layer sends the chords
            noteTable[i + PadButton.PAD_NOTE_OFFSET] = -1;
         }
      }
      getDriver().getNoteInput().setKeyTranslationTable(noteTable);
   }

   @Override
   protected String getModeDescription() {
      return chordMode ? "Chord Mode" : "Keyboard Mode";
   }

   public int getBaseNote() {
      return 60 + octaveOffset * 12 + baseNote;
   }

}
