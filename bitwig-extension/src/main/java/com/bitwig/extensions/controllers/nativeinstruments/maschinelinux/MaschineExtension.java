package com.bitwig.extensions.controllers.nativeinstruments.maschinelinux;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Iterator;
import java.util.LinkedList;
import java.util.Queue;
import java.util.function.Consumer;

import com.bitwig.extension.controller.ControllerExtension;
import com.bitwig.extension.controller.ControllerExtensionDefinition;
import com.bitwig.extension.api.util.midi.ShortMidiMessage;
import com.bitwig.extension.callback.ShortMidiMessageReceivedCallback;
import com.bitwig.extension.controller.api.Action;
import com.bitwig.extension.controller.api.CursorRemoteControlsPage;
import com.bitwig.extension.controller.api.HardwareActionBindable;
import com.bitwig.extension.controller.api.Application;
import com.bitwig.extension.controller.api.Arranger;
import com.bitwig.extension.controller.api.ClipLauncherSlot;
import com.bitwig.extension.controller.api.ClipLauncherSlotBank;
import com.bitwig.extension.controller.api.ControllerHost;
import com.bitwig.extension.controller.api.CursorDeviceFollowMode;
import com.bitwig.extension.controller.api.CursorTrack;
import com.bitwig.extension.controller.api.DeviceBank;
import com.bitwig.extension.controller.api.HardwareAction;
import com.bitwig.extension.controller.api.HardwareButton;
import com.bitwig.extension.controller.api.HardwareSurface;
import com.bitwig.extension.controller.api.MasterTrack;
import com.bitwig.extension.controller.api.MidiIn;
import com.bitwig.extension.controller.api.MidiOut;
import com.bitwig.extension.controller.api.NoteInput;
import com.bitwig.extension.controller.api.OnOffHardwareLight;
import com.bitwig.extension.controller.api.PinnableCursorDevice;
import com.bitwig.extension.controller.api.PopupBrowser;
import com.bitwig.extension.controller.api.Preferences;
import com.bitwig.extension.controller.api.Project;
import com.bitwig.extension.controller.api.RelativeHardwareKnob;
import com.bitwig.extension.controller.api.Send;
import com.bitwig.extension.controller.api.SendBank;
import com.bitwig.extension.controller.api.SettableEnumValue;
import com.bitwig.extension.controller.api.Track;
import com.bitwig.extension.controller.api.TrackBank;
import com.bitwig.extension.controller.api.Transport;
import com.bitwig.extensions.controllers.nativeinstruments.commons.ModifierState;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.buttons.GroupButton;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.buttons.ModeButton;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.buttons.PadButton;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.ArpDisplayLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.AutoDisplayLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.BrowserLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.DeviceLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.DisplayLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.MixerLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.PadModeDisplayLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.ScaleLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display.StepEditDisplayLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.ColorSelectionMode;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.DrumPadMode;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.GroupLayer;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.JogWheelDestination;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.KeyboardMode;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.MainKnobControl;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.PadMode;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.SceneLaunchMode;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.SessionMode;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.StepMode;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.VeloctiyHandler;
import com.bitwig.extensions.framework.Layers;
import com.bitwig.extensions.framework.time.TimedEvent;
import com.bitwig.extensions.framework.values.BooleanValueObject;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Iterator;
import java.util.LinkedList;
import java.util.Queue;
import java.util.function.Consumer;

public class MaschineExtension extends ControllerExtension implements JogWheelDestination {

   private final byte[] displayBuffer = { //
      (byte) 240, 0, 0, 102, 23, 18, 0, // 6: the grid number 0-3 * 28
      0, 0, 0, 0, 0, 0, 0, 0, // 7: 27 Chars
      0, 0, 0, 0, 0, 0, 0, 0, // 15:
      0, 0, 0, 0, 0, 0, 0, 0, // 22:
      0, 0, 0, 0, // 28:
      (byte) 247};
   private final String[] lastSenGrids = new String[4];
   private Transport transport;
   private MaschineLayer mainLayer;
   private MaschineLayer globalShiftLayer;

   private MidiOut midiOut;
   private MidiIn midiIn;
   private HardwareSurface surface;
   private Layers layers;
   private PadButton[] padButtons;
   private GroupButton[] groupButtons;
   private TrackBank trackBank;
   private TrackBank mixerTrackBank;
   private CursorTrack cursorTrack;
   private MasterTrack masterTrack;

   private PadMode fallbackAfterMomentaryMode;
   private PadMode currentMode;

   private DisplayLayer currentDisplayMode;
   private DisplayLayer lastControlDisplay;

   private ControllerHost host;
   private MainKnobControl mainKnobControl;
   private RelativeHardwareKnob[] displayKnobs;

   private HardwareButton[] touchButtons;
   private ModeButton[] displayButtons;
   private DeviceBank deviceBank;

   private int blinkState = 0;
   private boolean shiftDown = false;
   private boolean stopDown = false;
   private boolean eraseButtonDown = false;
   private Application application;
   private GroupLayer groupLayer;
   private NoteInput noteInput;
   private NoteInput extrasInput;
   private enum StripMode { NONE, PITCH, PERFORM }
   private StripMode stripMode = StripMode.NONE;
   private final BooleanValueObject stripPitch = new BooleanValueObject();
   private final BooleanValueObject stripPerform = new BooleanValueObject();
   private final BooleanValueObject inEditLayout = new BooleanValueObject();
   private CursorRemoteControlsPage stripRemote;
   private long duplicateDownTime = 0;
   private long selectDownTime = 0;
   private boolean allSelected = false;
   private boolean arpLatched = false;
   private final String[] lastValueRows = {null, null};

   private ModeButton noteRepeatButton;
   private ModeButton navLeftButton;
   private ModeButton navRightButton;

   private PinnableCursorDevice cursorDevice;
   private FocusClip focusClip;

   private final MaschineMode maschineMode;
   private ColorSelectionMode colorSelectionMode;
   private DrumPadMode drumPadMode;
   private KeyboardMode keyboardMode;
   private PinnableCursorDevice primaryDevice;
   private LayoutType currentLayoutType;
   private final BooleanValueObject inArrangeMode = new BooleanValueObject();
   private TouchHandler touchHandler;

   private final String[] displayBackupFields = new String[4];
   private long lastTempUpdate = 0;
   private PopupBrowser browser;
   private BrowserLayer browserLayer;
   private AutoDisplayLayer autoDisplayLayer;
   private Project project;
   private BooleanValueObject launchModifierSet = new BooleanValueObject();
   private SessionMode sessionMode;

   private final Queue<TimedEvent> timedEvents = new LinkedList<>();


   protected MaschineExtension(final ControllerExtensionDefinition definition, final ControllerHost host,
                               final MaschineMode mode) {
      super(definition, host);
      this.maschineMode = mode;
   }

   public HardwareSurface getSurface() {
      return surface;
   }

   public MidiOut getMidiOut() {
      return midiOut;
   }

   public MidiIn getMidiIn() {
      return midiIn;
   }

   public Application getApplication() {
      return application;
   }

   @Override
   public void init() {
      host = getHost();
      DebugOutMs.host = host;
      surface = host.createHardwareSurface();
      transport = host.createTransport();
      application = host.createApplication();
      project = host.getProject();

      midiOut = host.getMidiOutPort(0);
      midiIn = host.getMidiInPort(0);

      noteInput = midiIn.createNoteInput("MIDI", "80????", "90????", "A0????");
      noteInput.setShouldConsumeEvents(false);
      // Linux port: chords and touch strip pitch bend are generated here, on MIDI channel 16,
      // which the hardware never sends, so nothing arrives twice.
      extrasInput = midiIn.createNoteInput("Chords+Strip", "8F????", "9F????", "EF????");
      extrasInput.setShouldConsumeEvents(false);

      padButtons = new PadButton[16];
      for (int i = 0; i < 16; i++) {
         padButtons[i] = new PadButton(i, this);
      }
      groupButtons = new GroupButton[8];
      for (int i = 0; i < 8; i++) {
         groupButtons[i] = new GroupButton(i, this);
      }

      navLeftButton = new ModeButton(this, "LEFT_NAV_BUTTON", CcAssignment.NAV_LEFT);
      navRightButton = new ModeButton(this, "RIGHT_NAV_BUTTON", CcAssignment.NAV_RIGHT);

      initCursorTrack();
      initTrackBank();
      initDisplaySection();

      createLayers();
      setUpPreferences();

      mainKnobControl = new MainKnobControl(this);
      colorSelectionMode = new ColorSelectionMode(this);

      setUpMidiSysExCommands();
      currentDisplayMode.activate();
      currentMode.activate();
      groupLayer.activate();
      mainLayer.activate();
      mainKnobControl.activate();

      host.showPopupNotification(maschineMode.getDescriptor() + " Initialized");
      host.scheduleTask(this::handleBlink, 100);
      host.scheduleTask(this::handleTempDisplay, 200);
   }

   private void setUpMidiSysExCommands() {
      midiIn.setSysexCallback(data -> {
         final SysexCommandType command = maschineMode.toCommandType(data);
         switch (command) {
            case SHIFT_DOWN:
               mainLayer.deactivate();
               globalShiftLayer.activate();
               shiftDown = true;
               currentMode.setModifierState(ModifierState.SHIFT, true);
               groupLayer.setModifierState(ModifierState.SHIFT, true);
               break;
            case SHIFT_UP:
               shiftDown = false;
               globalShiftLayer.deactivate();
               mainLayer.activate();
               currentMode.setModifierState(ModifierState.SHIFT, false);
               groupLayer.setModifierState(ModifierState.SHIFT, false);
               break;
            case RETURN_FROM_HOST:
               surface.invalidateHardwareOutputState();
               host.requestFlush();
               break;
            case UNKNOWN:
               break;
            default:
               break;

         }
      });
   }

   public void setUpPreferences() {
      final Preferences preferences = getHost().getPreferences(); // THIS
      final SettableEnumValue focusMode = preferences.getEnumSetting("Focus", //
         "Recording/Automation", new String[]{FocusMode.LAUNCHER.getDescriptor(), FocusMode.ARRANGER.getDescriptor()},
         FocusMode.LAUNCHER.getDescriptor());
      focusMode.markInterested();
      final HardwareButton recordButton = createTransportButton("RECD", CcAssignment.RECORD);
      final HardwareButton autoButton = createHoldTransportButton("AUTO", CcAssignment.AUTO);
      final MaschineLayer arrangeFocusLayer = new MaschineLayer(this, "ArrangeFocus");
      final MaschineLayer sessionFocusLayer = new MaschineLayer(this, "SeesionFocus");

      arrangeFocusLayer.bindToggle(recordButton, transport.isArrangerRecordEnabled());
      // Linux port: AUTO = automation editor, SHIFT + AUTO = write automation (the old AUTO).
      arrangeFocusLayer.bindPressed(autoButton, () -> {
         if (shiftDown) {
            transport.isArrangerAutomationWriteEnabled().toggle();
         } else {
            showAutomation();
         }
      });
      arrangeFocusLayer.bindLightState(autoButton, transport.isArrangerAutomationWriteEnabled());

      sessionFocusLayer.bindPressed(recordButton, () -> {
         if (shiftDown) {
            transport.isClipLauncherOverdubEnabled().toggle();
         } else {
            focusClip.invokeRecord();
         }
      });
      sessionFocusLayer.bindLightState(recordButton, transport.isClipLauncherOverdubEnabled());
      sessionFocusLayer.bindIsPressed(autoButton, pressed -> handleAutoButton(pressed));
      sessionFocusLayer.bindLightState(autoButton, transport.isClipLauncherAutomationWriteEnabled());

      focusMode.addValueObserver(newValue -> {
         final FocusMode newMode = FocusMode.toMode(newValue);
         switch (newMode) {
            case ARRANGER:
               sessionFocusLayer.deactivate();
               arrangeFocusLayer.activate();
               break;
            case LAUNCHER:
               arrangeFocusLayer.deactivate();
               sessionFocusLayer.activate();
               break;
            default:
               break;
         }
      });
   }

   private long autoDownTime = 0;

   private boolean autoOpenedEditor = false;

   private void handleAutoButton(boolean pressed) {
      // Linux port: without SHIFT, AUTO opens/closes the automation editor.
      if (pressed && !shiftDown) {
         autoOpenedEditor = true;
         showAutomation();
         return;
      }
      if (!pressed && autoOpenedEditor) {
         autoOpenedEditor = false;
         return;
      }
      if (pressed) {
         autoDownTime = System.currentTimeMillis();
         setDisplayMode(autoDisplayLayer);
      } else {
         if ((System.currentTimeMillis() - autoDownTime) < 300) {
            transport.isClipLauncherAutomationWriteEnabled().toggle();
         }
         backToPreviousDisplayMode();
      }
   }

   void setUpArrangerHandling(final SessionMode sessionMode) {
      final HardwareButton arrangerButton = createTransportButton("ARRANGE", CcAssignment.ARRANGER);
      final HardwareButton followButton = createTransportButton("FOLLOW", CcAssignment.FOLLOWGRID);
      final HardwareButton saveButton = createTransportButton("SAVE", CcAssignment.SAVE);
      final HardwareButton eventsButton = createHoldTransportButton("EVENTS_BUTTON", CcAssignment.EVENTS);

      currentLayoutType = LayoutType.LAUNCHER;
      application.panelLayout().addValueObserver(v -> {
         currentLayoutType = LayoutType.toType(v);
         inArrangeMode.set(currentLayoutType == LayoutType.ARRANGER);
         sessionMode.notifyPanelLayout(currentLayoutType);
         mainKnobControl.notifyPanelLayout(currentLayoutType);
      });
      mainLayer.bindIsPressed(arrangerButton, v -> {
         application.setPanelLayout(currentLayoutType.other().getName());
      });
      mainLayer.bindLightState(arrangerButton, inArrangeMode);
      // Linux port additions: SHIFT + ARRANGER = note editor (piano roll), SETTINGS = automation editor.
      globalShiftLayer.bindPressed(arrangerButton, () -> application.toggleNoteEditor());
      final HardwareButton settingsButton = createTransportButton("SETTINGS", CcAssignment.SETTINGS);
      mainLayer.bindPressed(settingsButton, () -> application.toggleInspector());
      // SHIFT + SETTINGS: project panel ("Commuter panneau Projet" in Bitwig 6.1)
      final Action songPanel = firstAction("focus_or_toggle_song_panel", "toggle_song_panel");
      if (songPanel != null) {
         globalShiftLayer.bindPressed(settingsButton, (HardwareActionBindable) songPanel);
      }
      final Arranger arranger = host.createArranger();
      arranger.isClipLauncherVisible().markInterested();

      final Action followPlayHeadAction = application.getAction("toggle_playhead_follow");
      mainLayer.bindPressed(followButton, followPlayHeadAction);

      // Linux port: EVENTS = detail editor, SHIFT + EVENTS = inspector (edit the selected events).
      bindAction(mainLayer, eventsButton, "focus_or_toggle_detail_editor", () -> application.toggleNoteEditor());
      globalShiftLayer.bindPressed(eventsButton, () -> application.setPanelLayout("EDIT"));

      final Action saveAction = application.getAction("Save");
      final Action openAction = application.getAction("Open");
      globalShiftLayer.bindPressed(saveButton, openAction);
      mainLayer.bindPressed(saveButton, saveAction);
      globalShiftLayer.bindPressed(followButton, arranger.isClipLauncherVisible());
   }

   public void initBrowserSection() {
      browser = getHost().createPopupBrowser();
      browser.exists().markInterested();
      browserLayer = new BrowserLayer(this, "browser-display-mode");

      final HardwareButton browserButton = createTransportButton("BROWSERBUTTON", CcAssignment.BROWSER);
      mainLayer.bindPressed(browserButton, pressed -> {
         if (browser.exists().get()) {
            if (currentDisplayMode == browserLayer) {
               browser.cancel();
            } else {
               setDisplayMode(browserLayer);
            }
         } else {
            if (cursorDevice.exists().get()) {
               cursorDevice.afterDeviceInsertionPoint().browse();
            } else {
               deviceBank.browseToInsertDevice(0);
            }
         }
      });
      // Linux port: SHIFT + BROWSER shows/hides Bitwig's browser panel on the right.
      final Action browserPanel = firstAction("focus_or_toggle_browser_panel", "toggle_browser_panel");
      globalShiftLayer.bindPressed(browserButton, () -> {
         if (browserPanel != null) {
            browserPanel.invoke();
         } else {
            application.toggleBrowserVisibility();
         }
      });
      browser.exists().addValueObserver(exists -> {
         if (exists) {
            setDisplayMode(browserLayer);
         } else {
            backToPreviousDisplayMode();
         }
      });
      mainLayer.bindLightState(browserButton, browser.exists());
      globalShiftLayer.bindLightState(browserButton, browser.exists());
   }

   public int getBlinkState() {
      return blinkState;
   }

   void handleBlink() {
      blinkState = (blinkState + 1) % 8;
      if (!timedEvents.isEmpty()) {
         Iterator<TimedEvent> it = timedEvents.iterator();
         while (it.hasNext()) {
            final TimedEvent event = it.next();
            event.process();
            if (event.isCompleted()) {
               it.remove();
            }
         }
      }
      host.scheduleTask(this::handleBlink, 100);
   }

   public void queueTimedEvent(TimedEvent event) {
      timedEvents.add(event);
   }

   public void backToPreviousDisplayMode() {
      currentDisplayMode.deactivate();
      sendValuesRow(0, "");
      sendValuesRow(1, "");
      if (currentMode.getAssociatedDisplay() != null) {
         setDisplayMode(currentMode.getAssociatedDisplay());
      } else if (currentMode.isPrefersControlDisplay()) {
         setDisplayMode(lastControlDisplay);
      }
      currentDisplayMode.activate();
   }

   public void setDisplayMode(final DisplayLayer mode) {
      if (currentDisplayMode == mode) {
         return;
      }
      currentDisplayMode.deactivate();
      if (currentDisplayMode.isControlDisplay()) {
         lastControlDisplay = currentDisplayMode;
      }

      currentDisplayMode = mode;
      sendValuesRow(0, "");
      sendValuesRow(1, "");
      currentDisplayMode.activate();
   }

   public void enterColorSelection(final Consumer<BitWigColor> colorAction) {
      colorSelectionMode.setReturnMode(currentMode);
      colorSelectionMode.setColorAction(colorAction);
      setMode(colorSelectionMode);
   }

   public void cancelColorMode() {
      final PadMode returnmode = colorSelectionMode.getReturnMode();
      if (returnmode != null) {
         returnmode.setModifierState(ModifierState.VARIATION, false);
         setMode(returnmode);
         colorSelectionMode.setReturnMode(null);
         colorSelectionMode.setColorAction(null);
      }
   }

   final public void setMode(final PadMode mode) {
      if (currentMode == mode || mode == null) {
         return;
      }
      currentMode.deactivate();
      boolean forcePreferred = false;
      if (currentMode.getAssociatedDisplay() != null) {
         forcePreferred = true;
      }
      currentMode = mode;

      if (currentMode.getAssociatedDisplay() != null) {
         setDisplayMode(currentMode.getAssociatedDisplay());
      } else if (forcePreferred //
         && currentMode.isPrefersControlDisplay() //
         && !currentDisplayMode.isControlDisplay()) {
         setDisplayMode(lastControlDisplay);
      }
      currentMode.activate();
   }

   public DisplayLayer getCurrentDisplayMode() {
      return currentDisplayMode;
   }

   public DeviceBank getDeviceBank() {
      return deviceBank;
   }

   public ModeButton getNavLeftButton() {
      return navLeftButton;
   }

   public ModeButton getNavRightButton() {
      return navRightButton;
   }

   public NoteInput getNoteInput() {
      return noteInput;
   }

   public boolean isShiftDown() {
      return shiftDown;
   }

   public boolean isStopDown() {
      return stopDown;
   }

   public PadButton[] getPadButtons() {
      return padButtons;
   }

   public GroupButton[] getGroupButtons() {
      return groupButtons;
   }

   public TrackBank getTrackBank() {
      return trackBank;
   }

   public TrackBank getMixerTrackBank() {
      return mixerTrackBank;
   }

   public PinnableCursorDevice getCursorDevice() {
      return cursorDevice;
   }

   public PopupBrowser getBrowser() {
      return browser;
   }

   public Project getProject() {
      return project;
   }

   public BooleanValueObject getLaunchModifierSet() {
      return launchModifierSet;
   }

   /** Binds a Bitwig action by id; if this Bitwig version doesn't know it, uses the fallback. */
   private void bindAction(final MaschineLayer layer, final HardwareButton button, final String actionId,
                           final Runnable fallback) {
      final Action action = application.getAction(actionId);
      if (action != null) {
         layer.bindPressed(button, (HardwareActionBindable) action);
      } else {
         layer.bindPressed(button, fallback);
      }
   }

   /** The first of these Bitwig action ids that exists in this Bitwig version, or null. */
   private Action firstAction(final String... ids) {
      for (final String id : ids) {
         final Action action = application.getAction(id);
         if (action != null) {
            return action;
         }
      }
      return null;
   }

   /** First Bitwig action whose id or name contains all the words of one of the patterns. */
   private Action findAction(final String[]... patterns) {
      final Action[] actions = application.getActions();
      for (final String[] words : patterns) {
         for (final Action action : actions) {
            final String key = (action.getId() + " " + action.getName()).toLowerCase();
            boolean all = true;
            for (final String w : words) {
               all &= key.contains(w);
            }
            if (all) {
               return action;
            }
         }
      }
      return null;
   }

   /** Binds the action found by `patterns`, else `fallback` (may be null: does nothing). */
   private void bindFoundAction(final MaschineLayer layer, final HardwareButton button, final String what,
                                final Runnable fallback, final String[]... patterns) {
      final Action action = findAction(patterns);
      if (action != null) {
         host.println("Maschine Mk3 (Linux): " + what + " -> action " + action.getId() + " (" + action.getName() + ")");
         layer.bindPressed(button, () -> {
            action.invoke();
            host.showPopupNotification(action.getName());
         });
      } else {
         host.println("Maschine Mk3 (Linux): " + what + " -> no matching action"
            + (fallback != null ? ", using the fallback" : ""));
         if (fallback != null) {
            layer.bindPressed(button, fallback);
         }
      }
   }

   private Action automationAction;

   /** AUTO: Bitwig's own "show automation" action if it has one, else the API call. */
   private void showAutomation() {
      if (automationAction != null) {
         automationAction.invoke();
         host.showPopupNotification(automationAction.getName());
      } else {
         application.toggleAutomationEditor();
         host.showPopupNotification("Automation Editor");
      }
   }

   /**
    * Lists every Bitwig action in the controller console and in ~/maschine-mk3-bitwig-actions.txt,
    * to find the ids for buttons the API has no direct call for.
    */
   private void printActions() {
      final StringBuilder all = new StringBuilder();
      for (final Action action : application.getActions()) {
         all.append(action.getId()).append(" | ").append(action.getName()).append('\n');
      }
      host.println("---- Maschine Mk3 (Linux): Bitwig actions (id | name) ----");
      host.println(all.toString());
      host.println("---- end of actions ----");
      try {
         final java.nio.file.Path file = java.nio.file.Paths.get(System.getProperty("user.home"),
            "maschine-mk3-bitwig-actions.txt");
         java.nio.file.Files.writeString(file, all.toString());
         host.println("Actions written to " + file);
      } catch (final Exception e) {
         host.println("Could not write the actions file: " + e);
      }
   }

   /** New audio track after the selected one, then select and arm it. */
   private void createArmedAudioTrack() {
      final int position = cursorTrack.position().get() + 1;
      application.createAudioTrack(position);
      host.showPopupNotification("New Audio Track (armed)");
      host.scheduleTask(() -> {
         if (cursorTrack.position().get() != position) {
            cursorTrack.selectNext();
         }
         host.scheduleTask(() -> cursorTrack.arm().set(true), 150);
      }, 200);
   }

   /** Linux port: the knob values line under each screen (4 values separated by '|'). */
   public void sendValuesRow(final int screen, final String text) {
      if (text.equals(lastValueRows[screen])) {
         return;
      }
      lastValueRows[screen] = text;
      final byte[] chars = text.getBytes(StandardCharsets.US_ASCII);
      final int n = Math.min(chars.length, 56);
      final byte[] sysex = new byte[8 + n];
      sysex[0] = (byte) 0xF0;
      sysex[1] = 0;
      sysex[2] = 0;
      sysex[3] = 0x66;
      sysex[4] = 0x17;
      sysex[5] = 0x13;
      sysex[6] = (byte) screen;
      for (int i = 0; i < n; i++) {
         sysex[7 + i] = (byte) (chars[i] & 0x7F);
      }
      sysex[7 + n] = (byte) 0xF7;
      midiOut.sendSysex(sysex);
   }

   /** Linux port: buttons the original extension leaves unused. */
   private void setUpLinuxButtons() {
      // NOTES: Bitwig's EDIT view (ARRANGER switches between ARRANGE and MIX)
      final HardwareButton notesButton = createTransportButton("NOTES", CcAssignment.NOTES);
      application.panelLayout().addValueObserver(v -> inEditLayout.set("EDIT".equals(v)));
      mainLayer.bindPressed(notesButton, () -> application.setPanelLayout("EDIT"));
      mainLayer.bindLightState(notesButton, inEditLayout);

      // CHANNEL: device panel (SHIFT + CHANNEL stays the driver's MIDI mode toggle)
      final HardwareButton channelButton = createTransportButton("CHANNEL", CcAssignment.CHANNEL);
      bindAction(mainLayer, channelButton, "focus_or_toggle_device_panel", () -> application.toggleDevices());

      // SAMPLING: new audio track after the selected one, armed; SHIFT + SAMPLING: new effect track
      final HardwareButton samplingButton = createTransportButton("SAMPLING", CcAssignment.SAMPLING);
      cursorTrack.position().markInterested();
      mainLayer.bindPressed(samplingButton, this::createArmedAudioTrack);
      mainLayer.bindLightState(samplingButton, cursorTrack.arm());
      globalShiftLayer.bindPressed(samplingButton, () -> {
         application.createEffectTrack(-1);
         host.showPopupNotification("New Effect Track");
      });

      // LOCK: pin the selected device and track
      final HardwareButton lockButton = createTransportButton("LOCK", CcAssignment.LOCK);
      cursorDevice.isPinned().markInterested();
      cursorTrack.isPinned().markInterested();
      mainLayer.bindPressed(lockButton, () -> {
         final boolean pin = !cursorDevice.isPinned().get();
         cursorDevice.isPinned().set(pin);
         cursorTrack.isPinned().set(pin);
         host.showPopupNotification(pin ? "Locked" : "Unlocked");
      });
      mainLayer.bindLightState(lockButton, cursorDevice.isPinned());

      // PITCH / PERFORM: touch strip mode (pitch bend / 1st remote control of the device)
      stripRemote = cursorDevice.createCursorRemoteControlsPage("strip", 1, null);
      final HardwareButton pitchButton = createTransportButton("PITCH", CcAssignment.PITCH);
      final HardwareButton performButton = createTransportButton("PERFORM", CcAssignment.PERFORM);
      mainLayer.bindPressed(pitchButton, () -> setStripMode(StripMode.PITCH));
      mainLayer.bindPressed(performButton, () -> setStripMode(StripMode.PERFORM));
      mainLayer.bindLightState(pitchButton, stripPitch);
      mainLayer.bindLightState(performButton, stripPerform);
      midiIn.setMidiCallback((ShortMidiMessageReceivedCallback) this::onStripMidi);

      // MOD: show the modulators. The API has no direct call: use a Bitwig action whose id/name
      // mentions modulators, if this Bitwig version has one; otherwise the device's side panel.
      final HardwareButton modButton = createTransportButton("MOD", CcAssignment.MOD);
      cursorDevice.isRemoteControlsSectionVisible().markInterested();
      bindFoundAction(mainLayer, modButton, "Modulators", () -> {
         cursorDevice.isExpanded().set(true);
         cursorDevice.isRemoteControlsSectionVisible().toggle();
      }, new String[]{"modulator", "toggle"}, new String[]{"modulator", "show"}, new String[]{"modulators"});

      // CHORDS: chord mode on the pads (in keyboard mode)
      final ModeButton chordButton = new ModeButton(this, "CHORDS", CcAssignment.CHORD);
      mainLayer.bindPressed(chordButton.getHwButton(), () -> {
         keyboardMode.setChordMode(!keyboardMode.isChordMode());
         setMode(keyboardMode);
         host.showPopupNotification(keyboardMode.isChordMode() ? "Chords On" : "Chords Off");
      });
      mainLayer.bindLightState(chordButton, () -> keyboardMode.isChordMode() && currentMode == keyboardMode);
   }

   private void setStripMode(final StripMode mode) {
      stripMode = stripMode == mode ? StripMode.NONE : mode;
      stripPitch.set(stripMode == StripMode.PITCH);
      stripPerform.set(stripMode == StripMode.PERFORM);
      centerPitchBend();
      host.showPopupNotification("Touch Strip: " + stripMode);
   }

   private void centerPitchBend() {
      extrasInput.sendRawMidiEvent(0xEF, 0, 64);
   }

   /** Touch strip from the driver: CC 1 = position 0..127, CC 2 = touched (127) / released (0). */
   private void onStripMidi(final ShortMidiMessage msg) {
      if (msg.getStatusByte() != 0xB0) {
         return;
      }
      final int cc = msg.getData1();
      final int value = msg.getData2();
      if (cc == 1) {
         if (stripMode == StripMode.PITCH) {
            final int bend = value == 64 ? 8192 : value * 16383 / 127;
            extrasInput.sendRawMidiEvent(0xEF, bend & 0x7F, bend >> 7);
         } else if (stripMode == StripMode.PERFORM) {
            stripRemote.getParameter(0).value().set(value / 127.0);
         }
      } else if (cc == 2 && value == 0 && stripMode == StripMode.PITCH) {
         centerPitchBend(); // a pitch wheel springs back to the middle
      }
   }

   public NoteInput getExtrasInput() {
      return extrasInput;
   }

   public void handleShiftAction(final int padButtonIndex) {
      switch (padButtonIndex) {
         case PadButton.UNDO:
            application.undo();
            host.showPopupNotification("Undo");
            break;
         case PadButton.REDO:
            application.redo();
            host.showPopupNotification("Redo");
            break;
         case PadButton.QUANTIZE:
            application.focusPanelBelow();
            application.selectAll();
            host.showPopupNotification("Quantize");
            focusClip.quantize(1.0);
            break;
         case PadButton.QUANTIZE_50:
            application.focusPanelBelow();
            application.selectAll();
            host.showPopupNotification("Quantize");
            focusClip.quantize(0.5);
            break;
         case PadButton.COPY:
            application.copy();
            break;
         case PadButton.PASTE:
            application.paste();
            break;
         case PadButton.CLEAR:
            focusClip.clearSteps();
            break;
         case PadButton.CLEAR_AUTO:
            // Bitwig has no API to erase a clip's automation: give control back to it instead.
            transport.resetAutomationOverrides();
            host.showPopupNotification("Restore Automation");
            break;
         case PadButton.STEP_UNDO:
            application.undo();
            host.showPopupNotification("Undo");
            break;
         case PadButton.STEP_REDO:
            application.redo();
            host.showPopupNotification("Redo");
            break;
         case PadButton.NUDGE_LEFT:
            application.arrowKeyLeft();
            host.showPopupNotification("Nudge <");
            break;
         case PadButton.NUDGE_RIGHT:
            application.arrowKeyRight();
            host.showPopupNotification("Nudge >");
            break;
         case PadButton.SEMI_PLUS:
            focusClip.transpose(1);
            break;
         case PadButton.SEMI_MINUS:
            focusClip.transpose(-1);
            break;
         case PadButton.OCT_PLUS:
            focusClip.transpose(12);
            break;
         case PadButton.OCT_MINUS:
            focusClip.transpose(-12);
            break;
         default:
            break;
      }
   }

   public FocusClip getFocusClip() {
      return focusClip;
   }

   private void createLayers() {
      // We create all the layers here because the main layer might bind actions to
      // activate other layers.
      final Track rootTrack = project.getRootTrackGroup();

      layers = new Layers(this);
      mainLayer = new MaschineLayer(this, "Main");
      globalShiftLayer = new MaschineLayer(this, "Shift");

      final HardwareButton playButton = createTransportButton("PLAY", CcAssignment.PLAY);
      final HardwareButton stopButton = createHoldTransportButton("STOP", CcAssignment.STOP);
      final HardwareButton tapButton = createTransportButton("TAP", CcAssignment.TAPMETRO);
      final HardwareButton restartButton = createTransportButton("RESTART", CcAssignment.RESTART);

      mainLayer.bindToggle(playButton, transport.isPlaying());
      mainLayer.bindPressed(stopButton, transport.stopAction());
      mainLayer.bindPressed(stopButton, v -> stopDown = true);
      mainLayer.bindReleased(stopButton, v -> stopDown = false);
      mainLayer.bindPressed(tapButton, transport.tapTempoAction());
      mainLayer.bindPressed(restartButton, this::invokeRestart);

      globalShiftLayer.bindPressed(playButton, this::invokeRestart);
      globalShiftLayer.bindPressed(restartButton, transport.isArrangerLoopEnabled());
      globalShiftLayer.bindPressed(tapButton, transport.isMetronomeEnabled());
      globalShiftLayer.bind(transport.isMetronomeEnabled(), tapButton);
      globalShiftLayer.bind(transport.isArrangerLoopEnabled(), restartButton);
      globalShiftLayer.bindPressed(stopButton, rootTrack.stopAction());

      groupLayer = new GroupLayer(this, "group-layer");

      focusClip = new FocusClip(this);
      sessionMode = new SessionMode(this, "session-mode");
      final SceneLaunchMode sceneMode = new SceneLaunchMode(this, "scene-mode");

      final MixerLayer mixerDisplayMode = new MixerLayer(this, "mixer-display-mode");
      final DeviceLayer deviceDisplayMode = new DeviceLayer(this, "device-display-mode");
      final ArpDisplayLayer arpDisplayMode = new ArpDisplayLayer(this, "arp-display-mode");

      final ModeButton patternButton = new ModeButton(this, "PATTERN_MODE", CcAssignment.PATTERN);
      final ModeButton sceneButton = new ModeButton(this, "SCENE_MODE", CcAssignment.SCENE);

      final ModeButton duplicateButton = new ModeButton(this, "DUPLICATE", CcAssignment.DUPLICATE).bindToPressed();
      final ModeButton selectButton = new ModeButton(this, "SELECT", CcAssignment.SELECT).bindToPressed();
      final ModeButton soloButton = new ModeButton(this, "SOLO", CcAssignment.SOLO).bindToPressed();
      final ModeButton muteButton = new ModeButton(this, "MUTE", CcAssignment.MUTE).bindToPressed();
      final ModeButton eraseButton = new ModeButton(this, "ERASE", CcAssignment.ERASE).bindToPressed();
      final ModeButton variationButton = new ModeButton(this, "VARIATION", CcAssignment.VARIATION).bindToPressed();

      final ModeButton mixerButton = new ModeButton(this, "MIXER", CcAssignment.MIXER);
      final ModeButton deviceButton = new ModeButton(this, "DEVICE", CcAssignment.PLUGIN);

      final ModeButton macroButton = new ModeButton(this, "MACRO", CcAssignment.MACRO);

      macroButton.getHwButton().isPressed().addValueObserver(active -> {
         currentDisplayMode.notifyMacroDown(active);
      });
      mainLayer.bindLightState(macroButton, macroButton.getHwButton().isPressed());

      final Action aliasAction = firstAction("Duplicate Reference");
      duplicateButton.getHwButton().isPressed().addValueObserver(active -> {
         if (shiftDown && active) {
            // Linux port: SHIFT + DUPLICATE = duplicate as alias (if Bitwig has the action)
            if (aliasAction != null) {
               aliasAction.invoke();
               host.showPopupNotification(aliasAction.getName());
            } else {
               focusClip.duplicateContent();
            }
         } else if (!shiftDown) {
            setModifierState(ModifierState.DUPLICATE, active);
            // Linux port: a short tap duplicates the selection; held + pad/group as before
            if (active) {
               duplicateDownTime = System.currentTimeMillis();
            } else if (System.currentTimeMillis() - duplicateDownTime < 300) {
               application.duplicate();
            }
         }
      });
      selectButton.getHwButton().isPressed().addValueObserver(active -> {
         setSelectModifierState(active);
         // Linux port: a short tap selects all / nothing in the focused editor or arrangement
         if (active) {
            selectDownTime = System.currentTimeMillis();
         } else if (System.currentTimeMillis() - selectDownTime < 300) {
            allSelected = !allSelected;
            if (allSelected) {
               application.selectAll();
            } else {
               application.selectNone();
            }
         }
      });
      soloButton.getHwButton().isPressed().addValueObserver(active -> setModifierState(ModifierState.SOLO, active));
      muteButton.getHwButton().isPressed().addValueObserver(active -> setModifierState(ModifierState.MUTE, active));
      eraseButton.getHwButton().isPressed().addValueObserver(active -> {
         eraseButtonDown = active;
         setModifierState(ModifierState.ERASE, active);
      });
      // Linux port: VARIATION shows/hides the track's device panel (no more colour choice).
      final Action devicePanelAction = application.getAction("focus_or_toggle_device_panel");
      variationButton.getHwButton().isPressed().addValueObserver(active -> {
         if (active) {
            if (currentMode == colorSelectionMode) {
               cancelColorMode();
            }
            if (devicePanelAction != null) {
               devicePanelAction.invoke();
            } else {
               application.toggleDevices();
            }
         }
      });

      mainLayer.bindMode(patternButton, sessionMode);
      mainLayer.bindMode(sceneButton, sceneMode);

      setStepAndPlayingLayers();

      mainLayer.bindMode(deviceButton, deviceDisplayMode, false);
      mainLayer.bindMode(mixerButton, mixerDisplayMode, false);

      // Linux port additions: open Bitwig's own panels and plugin windows from the Maschine.
      mixerButton.getHwButton().isPressed().addValueObserver(pressed -> {
         if (pressed && !shiftDown) {
            application.toggleMixer();
         }
      });
      globalShiftLayer.bindPressed(mixerButton.getHwButton(), () -> application.setPanelLayout("MIX"));
      globalShiftLayer.bindPressed(deviceButton.getHwButton(), () -> cursorDevice.isWindowOpen().toggle());

      final HardwareButton touch4DButton = createTouchButton(midiIn, "NAV4D_TOUCH", CcAssignment.DKNOB_TOUCH.getCcNr());
      touch4DButton.isPressed().addValueObserver(touched -> {
         currentDisplayMode.notifyMainTouched(touched);
         mainKnobControl.setDisplayActive(touched);
      });

      currentMode = sessionMode;
      currentDisplayMode = mixerDisplayMode;
      lastControlDisplay = mixerDisplayMode;
      autoDisplayLayer = new AutoDisplayLayer(this, "AUTO DISPLAY LAYER");

      initNoteRepeat(arpDisplayMode);
      setUpArrangerHandling(sessionMode);
      initBrowserSection();
      setUpLinuxButtons();
      printActions();
      // ids from the user's Bitwig 6.1 action list
      automationAction = firstAction("toggle_automation_shown_for_selected_tracks", "show_automation_editor_mode");
      host.println("Maschine Mk3 (Linux): AUTO -> "
         + (automationAction != null ? automationAction.getId() : "toggleAutomationEditor()"));
   }

   public HardwareButton create4DSelectButton(final String name, final CcAssignment assignment) {
      final HardwareButton navdownButton = surface.createHardwareButton(name);
      navdownButton.pressedAction().setActionMatcher(assignment.createActionMatcher(midiIn, 127));
      navdownButton.releasedAction().setActionMatcher(assignment.createActionMatcher(midiIn, 0));
      final OnOffHardwareLight led = surface.createOnOffHardwareLight(name + "_LED");
      navdownButton.setBackgroundLight(led);
      return navdownButton;
   }

   private void initNoteRepeat(final ArpDisplayLayer arpDisplayMode) {
      noteRepeatButton = new ModeButton(this, "NOTE_REPEAT", CcAssignment.NOTE_REPEAT);
      mainLayer.bindModeMomentary(noteRepeatButton, arpDisplayMode, false);
      noteInput.arpeggiator().isEnabled().markInterested();
      noteRepeatButton.getHwButton().isPressed().addValueObserver(pressed -> {
         if (!primaryDevice.exists().get()) {
            return;
         }
         // Linux port: SHIFT + NOTE REPEAT latches the arpeggiator (knobs: its settings)
         if (shiftDown) {
            if (pressed) {
               arpLatched = !arpLatched;
               noteInput.arpeggiator().isEnabled().set(arpLatched);
               if (arpLatched) {
                  setMode(primaryDevice.hasDrumPads().get() ? drumPadMode : keyboardMode);
                  setDisplayMode(arpDisplayMode);
               } else {
                  backToPreviousDisplayMode();
               }
               host.showPopupNotification(arpLatched ? "Arp On" : "Arp Off");
            }
            return;
         }
         if (arpLatched) {
            return; // latched: plain NOTE REPEAT only shows the arp settings while held
         }
         noteInput.arpeggiator().isEnabled().set(pressed);
         if (pressed) {
            fallbackAfterMomentaryMode = currentMode;
            if (primaryDevice.hasDrumPads().get()) {
               setMode(drumPadMode);
            } else {
               setMode(keyboardMode);
            }
         } else {
            if (fallbackAfterMomentaryMode != null && fallbackAfterMomentaryMode != currentMode) {
               setMode(fallbackAfterMomentaryMode);
               fallbackAfterMomentaryMode = null;
            }
         }
      });
      noteRepeatButton.getLed().isOn().setValueSupplier(() -> noteInput.arpeggiator().isEnabled().get());
   }

   private void setStepAndPlayingLayers() {
      final VeloctiyHandler velocityHandler = new VeloctiyHandler(noteInput);
      final ScaleLayer scaleDisplayMode = new ScaleLayer(this, "scale-display-mode", velocityHandler);
      final PadModeDisplayLayer padDisplayLayer = new PadModeDisplayLayer(this, "pad-display-mode", velocityHandler);
      final StepEditDisplayLayer stepDisplayLayer = new StepEditDisplayLayer(this, "step-display-mode");

      final ModeButton padModeButton = new ModeButton(this, "PAD_MODE", CcAssignment.PADMODE);
      final ModeButton keyboardButton = new ModeButton(this, "KEYBOARD_MODE", CcAssignment.KEYBOARD);
      final ModeButton stepButton = new ModeButton(this, "STEP_MODE", CcAssignment.STEP);

      final StepMode stepMode = new StepMode(this, "step-mode");
      drumPadMode = new DrumPadMode(this, "pad-mode", stepMode, velocityHandler, padDisplayLayer);
      keyboardMode = new KeyboardMode(this, "keyboard-mode", stepMode, velocityHandler, scaleDisplayMode);

      stepMode.setDisplay(stepDisplayLayer);
      drumPadMode.setAltMode(keyboardMode);
      keyboardMode.setAltMode(drumPadMode);

      stepDisplayLayer.setStepLayer(stepMode);
      stepMode.setSelectLayers(drumPadMode, keyboardMode);

      final ModeButton fixedVelButton = new ModeButton(this, "FIXED_VELOCITY", CcAssignment.FIXEDVEL);

      mainLayer.bindPressed(fixedVelButton, () -> velocityHandler.toggleFixedValue());
      mainLayer.bindLightState(fixedVelButton, velocityHandler.getFixed());

      mainLayer.bindMode(padModeButton, drumPadMode);
      mainLayer.bindMode(keyboardButton, keyboardMode);
      mainLayer.bindMode(stepButton, stepMode);

      mainLayer.bindMode(keyboardButton, scaleDisplayMode, true); // Decoupled light
      mainLayer.bindMode(padModeButton, padDisplayLayer, true); // Decoupled light
      mainLayer.bindMode(stepButton, stepDisplayLayer, false);

   }

   private HardwareButton createHoldTransportButton(final String name, final CcAssignment assignment) {
      final HardwareButton button = surface.createHardwareButton(name + "_BUTTON");
      button.pressedAction().setActionMatcher(assignment.createActionMatcher(midiIn, 127));
      button.releasedAction().setActionMatcher(assignment.createActionMatcher(midiIn, 0));
      final OnOffHardwareLight led = surface.createOnOffHardwareLight(name + "BUTTON_LED");
      button.setBackgroundLight(led);
      led.onUpdateHardware(() -> {
         sendLedUpdate(assignment, led.isOn().currentValue() ? 127 : 0);
      });
      return button;
   }

   private HardwareButton createTransportButton(final String name, final CcAssignment assignment) {
      final HardwareButton playButton = surface.createHardwareButton(name + "_BUTTON");
      final HardwareAction action = playButton.pressedAction();
      action.setActionMatcher(assignment.createActionMatcher(midiIn, 127));
      final OnOffHardwareLight led = surface.createOnOffHardwareLight(name + "BUTTON_LED");
      playButton.setBackgroundLight(led);
      led.onUpdateHardware(() -> {
         sendLedUpdate(assignment, led.isOn().currentValue() ? 127 : 0);
      });
      return playButton;
   }

   private void setModifierState(final ModifierState modstate, final boolean active) {
      currentMode.setModifierState(modstate, active);
      groupLayer.setModifierState(modstate, active);
   }

   private void setSelectModifierState(final boolean active) {
      if (currentMode.hasMomentarySelectMode() || fallbackAfterMomentaryMode != null) {
         if (active) {
            fallbackAfterMomentaryMode = currentMode;
            setMode(currentMode.getMomentarySwitchMode());
            currentMode.setModifierState(ModifierState.SELECT, true);
         } else {
            currentMode.setModifierState(ModifierState.SELECT, false);
            setMode(fallbackAfterMomentaryMode);
            fallbackAfterMomentaryMode = null;
         }
      } else {
         currentMode.setModifierState(ModifierState.SELECT, active);
      }
      groupLayer.setModifierState(ModifierState.SELECT, active);
   }

   public ModeButton getNoteRepeatButton() {
      return noteRepeatButton;
   }

   public CursorTrack getCursorTrack() {
      return cursorTrack;
   }

   public RelativeHardwareKnob[] getDisplayKnobs() {
      return displayKnobs;
   }

   public ModeButton[] getDisplayButtons() {
      return displayButtons;
   }

   public HardwareButton[] getTouchButtons() {
      return touchButtons;
   }

   public TouchHandler getTouchHandler() {
      return touchHandler;
   }

   private void initDisplaySection() {
      for (int i = 0; i < lastSenGrids.length; i++) {
         lastSenGrids[i] = "----------------------";
      }

      displayKnobs = new RelativeHardwareKnob[8];
      displayButtons = new ModeButton[8];
      touchButtons = new HardwareButton[8];

      final CcAssignment[] buttonAssingments = {CcAssignment.MODE_BUTTON_1, CcAssignment.MODE_BUTTON_2, CcAssignment.MODE_BUTTON_3, CcAssignment.MODE_BUTTON_4, CcAssignment.MODE_BUTTON_5, CcAssignment.MODE_BUTTON_6, CcAssignment.MODE_BUTTON_7, CcAssignment.MODE_BUTTON_8};

      for (int i = 0; i < 8; i++) {
         final RelativeHardwareKnob knob = surface.createRelativeHardwareKnob("ENDLESKNOB_" + i);
         displayKnobs[i] = knob;
         knob.setAdjustValueMatcher(midiIn.createRelative2sComplementCCValueMatcher(0, 70 + i, 128));
         knob.setStepSize(1 / 64.0);
      }

      touchHandler = new TouchHandler();
      for (int i = 0; i < 8; i++) {
         final int index = i;
         final HardwareButton touchButton = createTouchButton(midiIn, "DISPLAY_TOUCH_" + i, 10 + i);
         touchButtons[i] = touchButton;
         touchButton.isPressed().addValueObserver(v -> {
            touchHandler.notifyTouched(index, v);
            currentDisplayMode.notifyTouched(index, v);
         });
      }

      for (int i = 0; i < displayButtons.length; i++) {
         displayButtons[i] = new ModeButton(this, "DISPLAY_BUTTON_" + i, buttonAssingments[i]);
      }
   }

   private void initCursorTrack() {
      cursorTrack = getHost().createCursorTrack(8, 32);
      cursorTrack.hasPrevious().markInterested();
      cursorTrack.hasNext().markInterested();
      cursorTrack.playingNotes().markInterested();
      cursorTrack.arm().markInterested();
      cursorTrack.exists().markInterested();
      cursorTrack.mute().markInterested();
      cursorTrack.solo().markInterested();
      cursorTrack.arm().markInterested();
      cursorTrack.volume().value().markInterested();
      cursorTrack.pan().value().markInterested();
      cursorTrack.isStopped().markInterested();
      cursorTrack.isQueuedForStop().markInterested();
      cursorTrack.isActivated().markInterested();

      final ClipLauncherSlotBank slotBank = cursorTrack.clipLauncherSlotBank();
      slotBank.canScrollForwards().markInterested();
      slotBank.scrollPosition().markInterested();
      for (int i = 0; i < slotBank.getCapacityOfBank(); i++) {
         final ClipLauncherSlot cs = slotBank.getItemAt(i);
         cs.hasContent().markInterested();
         cs.isSelected().markInterested();
      }

      masterTrack = getHost().createMasterTrack(8);
      masterTrack.volume().markInterested();

      deviceBank = cursorTrack.createDeviceBank(4);
      cursorDevice = cursorTrack.createCursorDevice();
      cursorDevice.exists().markInterested();
      primaryDevice = cursorTrack.createCursorDevice("drumdetection", "Pad Device", 0,
         CursorDeviceFollowMode.FIRST_INSTRUMENT);
      primaryDevice.hasDrumPads().markInterested();
      primaryDevice.exists().markInterested();
   }

   private void initTrackBank() {
      // mixerTrackBank = getHost().createTrackBank(8, 1, 0);
      mixerTrackBank = getHost().createMainTrackBank(8, 1, 1);
      mixerTrackBank.cursorIndex().markInterested();
      mixerTrackBank.setSkipDisabledItems(true);
      mixerTrackBank.canScrollChannelsDown().markInterested();
      mixerTrackBank.canScrollChannelsUp().markInterested();

      trackBank = getHost().createMainTrackBank(4, 4, 4);
      // trackBank.followCursorTrack(cursorTrack);
      trackBank.cursorIndex().markInterested();
      trackBank.setSkipDisabledItems(true);
      trackBank.canScrollChannelsDown().markInterested();
      trackBank.canScrollChannelsUp().markInterested();
      trackBank.setShouldShowClipLauncherFeedback(true);

      for (int i = 0; i < 8; i++) {
         final Track channel = mixerTrackBank.getItemAt(i);
         channel.exists().markInterested();
         channel.mute().markInterested();
         channel.solo().markInterested();
         channel.arm().markInterested();
         channel.volume().value().markInterested();
         channel.pan().value().markInterested();
         channel.color().markInterested();
         channel.isStopped().markInterested();
         channel.isQueuedForStop().markInterested();
         channel.isActivated().markInterested();
         channel.canHoldAudioData().markInterested();
         channel.canHoldNoteData().markInterested();
      }

      for (int i = 0; i < 4; ++i) {
         final Track channel = trackBank.getItemAt(i);
         channel.exists().markInterested();
         channel.mute().markInterested();
         channel.solo().markInterested();
         channel.arm().markInterested();
         channel.volume().value().markInterested();
         channel.pan().value().markInterested();
         channel.color().markInterested();
         channel.isStopped().markInterested();
         channel.isQueuedForStop().markInterested();

         final ClipLauncherSlotBank clipLauncherSlots = channel.clipLauncherSlotBank();

         final SendBank sendBank = channel.sendBank();
         sendBank.setSkipDisabledItems(true);

         for (int j = 0; j < 4; ++j) {
            final ClipLauncherSlot slot = clipLauncherSlots.getItemAt(j);
            slot.color().markInterested();
            slot.isPlaybackQueued().markInterested();
            slot.hasContent().markInterested();
            slot.isRecording().markInterested();
            slot.isPlaying().markInterested();
            slot.isRecordingQueued().markInterested();
            slot.isStopQueued().markInterested();
            slot.isSelected().markInterested();

            final Send send = sendBank.getItemAt(j);
            send.value().markInterested();
            send.sendChannelColor().markInterested();
            send.exists().markInterested();
         }
      }
   }

   private HardwareButton createTouchButton(final MidiIn midiIn, final String name, final int ccNr) {
      final HardwareButton button = surface.createHardwareButton(name);
      button.pressedAction().setActionMatcher(midiIn.createCCActionMatcher(0, ccNr, 127));
      button.releasedAction().setActionMatcher(midiIn.createCCActionMatcher(0, ccNr, 0));
      return button;
   }

   public void sendLedUpdate(final CcAssignment assingment, final int value) {
      midiOut.sendMidi(assingment.getType(), assingment.getCcNr(), value);
   }

   public void updatePadLed(final RgbButton button) {
      final RgbLed state = (RgbLed) button.getLight().state().currentValue();
      if (state != null) {
         midiOut.sendMidi(button.getMidiStatus(), button.getMidiDataNr(), state.getColor());
      } else {
         midiOut.sendMidi(button.getMidiStatus(), button.getMidiDataNr(), 0);
      }
   }

   public void handleTempDisplay() {
      if (lastTempUpdate > 0 && System.currentTimeMillis() - lastTempUpdate > 1000) {
         this.sendToDisplay(0, displayBackupFields[0]);
         lastTempUpdate = 0;
      }
      this.host.scheduleTask(this::handleTempDisplay, 300);
   }

   public void sendToDisplayTemp(final int grid, final String text) {
      lastTempUpdate = System.currentTimeMillis();
      sendToDisplay(grid, text);
   }

   public void sendToDisplayBuffered(final int grid, final String text) {
      displayBackupFields[grid] = text;
      sendToDisplay(grid, text);
      lastTempUpdate = 0;
   }

   public void sendToDisplay(final int grid, final String text) {
      if (text.equals(lastSenGrids[grid])) {
         return;
      }
      lastSenGrids[grid] = text;
      displayBuffer[6] = (byte) (Math.min(grid, 3) * 28);
      final byte[] ca = text.getBytes(StandardCharsets.US_ASCII);
      for (int i = 0; i < 28; i++) {
         displayBuffer[i + 7] = i < ca.length ? ca[i] : 32;
      }
      midiOut.sendSysex(displayBuffer);
   }

   @Override
   public void exit() {
      for (int i = 0; i < 4; i++) {
         sendToDisplay(i, "");
      }
      sendValuesRow(0, "");
      sendValuesRow(1, "");
      Arrays.stream(CcAssignment.values()).forEach(assignment -> {
         sendLedUpdate(assignment, 0);
      });
      for (final PadButton button : padButtons) {
         midiOut.sendMidi(button.getMidiStatus(), button.getMidiDataNr(), 0);
      }
      for (final GroupButton button : groupButtons) {
         midiOut.sendMidi(button.getMidiStatus(), button.getMidiDataNr(), 0);
      }
      getHost().showPopupNotification(maschineMode.getDescriptor() + " Exited");
      surface.updateHardware();
      // For the shutdown process to clear the display, we currently need
      // to call sleep() with 400ms, which is quite disruptive.
   }

   @Override
   public void flush() {
      surface.updateHardware();
   }

   public Layers getLayers() {
      return layers;
   }

   public MasterTrack getMasterTrack() {
      return masterTrack;
   }

   public Transport getTranport() {
      return transport;
   }

   public PinnableCursorDevice getPrimaryDevice() {
      return primaryDevice;
   }

   private void invokeRestart() {
      transport.stop();
      getHost().scheduleTask(() -> {
         transport.stop();
      }, 10);
      getHost().scheduleTask(() -> {
         transport.play();
      }, 20);
   }

   public boolean isEraseDown() {
      return eraseButtonDown;
   }

   public JogWheelDestination getJogWheelDestination() {
      if (browser.exists().get()) {
         return browserLayer;
      }
      if (currentMode instanceof JogWheelDestination) {
         return (JogWheelDestination) currentMode;
      }
      return this;
   }

   @Override
   public void jogWheelPush(final boolean push) {
   }

   @Override
   public void jogWheelAction(final int increment) {
      if (increment > 0) {
         transport.fastForward();
      } else {
         transport.rewind();
      }
   }

}
