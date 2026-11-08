package com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.display;

import com.bitwig.extension.controller.api.BrowserFilterColumn;
import com.bitwig.extension.controller.api.BrowserFilterItem;
import com.bitwig.extension.controller.api.BrowserItem;
import com.bitwig.extension.controller.api.BrowserItemBank;
import com.bitwig.extension.controller.api.BrowserResultsItemBank;
import com.bitwig.extension.controller.api.ControllerHost;
import com.bitwig.extension.controller.api.CursorBrowserFilterItem;
import com.bitwig.extension.controller.api.CursorBrowserItem;
import com.bitwig.extension.controller.api.CursorBrowserResultItem;
import com.bitwig.extension.controller.api.PopupBrowser;
import com.bitwig.extension.controller.api.RelativeHardwareKnob;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.MaschineExtension;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.buttons.ModeButton;
import com.bitwig.extensions.controllers.nativeinstruments.maschinelinux.modes.JogWheelDestination;

/**
 * Bitwig's pop-up browser on the Maschine.
 * <p>
 * Linux port, rewritten after DrivenByMoss (Jürgen Moßgraber, LGPLv3), whose browser works with
 * Bitwig 5/6: same filter columns in the same order, the column's item bank created before the
 * cursor used to navigate, and every value subscribed. The content type is not controllable since
 * Bitwig 5 (noted in DrivenByMoss as well), so it is left out.
 * <p>
 * Knobs 1..7 = filter columns (location, file type, category, tag, creator, device type, device),
 * knob 8 = results. The big encoder also scrolls the results, its push loads the selection, the 4th
 * button above the screens toggles audition.
 */
public class BrowserLayer extends DisplayLayer implements JogWheelDestination {

   /** Knob notches per step. */
   private static final int TICKS_PER_STEP = 2;
   /** Size of the item banks (subscribed items per column), as DrivenByMoss uses. */
   private static final int BANK_SIZE = 16;
   private static final int RESULTS = 7;

   private final PopupBrowser browser;
   private final ControllerHost host;
   private final String[] columnNames = new String[8];
   private final String[] selections = new String[8];
   private final boolean[] columnExists = new boolean[8];
   private final CursorBrowserResultItem cursorResult;
   private int touchedIndex = -1;
   private String lastKnob = "";

   public BrowserLayer(final MaschineExtension driver, final String name) {
      super(driver, name);
      browser = driver.getBrowser();
      host = driver.getHost();
      final ModeButton[] buttons = driver.getDisplayButtons();
      final RelativeHardwareKnob[] knobs = driver.getDisplayKnobs();
      for (int i = 0; i < 8; i++) {
         columnNames[i] = "";
         selections[i] = "";
         columnExists[i] = true;
      }

      browser.exists().markInterested();
      browser.selectedContentTypeIndex().markInterested();
      browser.selectedContentTypeName().markInterested();
      browser.contentTypeNames().markInterested();
      browser.shouldAudition().markInterested();

      final BrowserFilterColumn[] columns = {browser.locationColumn(), browser.fileTypeColumn(),
         browser.categoryColumn(), browser.tagColumn(), browser.creatorColumn(), browser.deviceTypeColumn(),
         browser.deviceColumn()};
      for (int i = 0; i < columns.length; i++) {
         final int index = i;
         final BrowserFilterColumn column = columns[i];

         column.exists().markInterested();
         column.name().markInterested();
         column.getWildcardItem().name().markInterested();
         column.name().addValueObserver(v -> {
            columnNames[index] = v;
            updateDisplay();
         });
         column.exists().addValueObserver(v -> {
            columnExists[index] = v;
            updateDisplay();
         });

         // as DrivenByMoss: the item bank's cursor first, then the cursor used for navigation
         final BrowserItemBank itemBank = ((CursorBrowserItem) column.createCursorItem()).createSiblingsBank(BANK_SIZE);
         itemBank.cursorIndex().markInterested();
         for (int j = 0; j < BANK_SIZE; j++) {
            subscribe((BrowserItem) itemBank.getItemAt(j));
         }

         final CursorBrowserFilterItem cursor = (CursorBrowserFilterItem) column.createCursorItem();
         cursor.exists().markInterested();
         cursor.name().markInterested();
         cursor.name().addValueObserver(v -> {
            selections[index] = v.trim();
            updateDisplay();
         });

         bind(knobs[index], createIncrementBinder(host, increment -> {
            knobTurned(index, increment);
            if (increment > 0) {
               cursor.selectNext();
            } else {
               cursor.selectPrevious();
            }
         }, TICKS_PER_STEP));
      }

      // results: knob 8 and the big encoder
      columnNames[RESULTS] = "Result";
      cursorResult = (CursorBrowserResultItem) browser.resultsColumn().createCursorItem();
      cursorResult.name().markInterested();
      cursorResult.name().addValueObserver(v -> {
         selections[RESULTS] = v.trim();
         updateDisplay();
      });
      final BrowserResultsItemBank resultsBank = (BrowserResultsItemBank) cursorResult.createSiblingsBank(BANK_SIZE);
      for (int j = 0; j < BANK_SIZE; j++) {
         subscribe((BrowserItem) resultsBank.getItemAt(j));
      }
      bind(knobs[RESULTS], createIncrementBinder(host, increment -> {
         knobTurned(RESULTS, increment);
         jogWheelAction(increment);
      }, TICKS_PER_STEP));

      bindPressed(buttons[3], browser.shouldAudition());
      bindLightState(buttons[3], browser.shouldAudition());
   }

   private static void subscribe(final BrowserItem item) {
      item.exists().markInterested();
      item.name().markInterested();
      item.isSelected().markInterested();
      if (item instanceof BrowserFilterItem) {
         ((BrowserFilterItem) item).hitCount().markInterested();
      }
   }

   private void knobTurned(final int index, final int increment) {
      lastKnob = "K" + (index + 1) + (increment > 0 ? "+" : "-");
      host.println("Maschine Mk3 (Linux): browser knob " + (index + 1) + (increment > 0 ? " +" : " -")
         + " column=" + columnNames[index] + " exists=" + columnExists[index] + " value=" + selections[index]);
      updateDisplay();
   }

   /** Names of 4 columns for one screen, as 4 fields of 6 chars separated by '|'. */
   private String names(final int from) {
      final StringBuilder b = new StringBuilder();
      for (int i = from; i < from + 4; i++) {
         b.append(DisplayUtil.padString(columnExists[i] ? columnNames[i] : "-", 6));
         if (i < from + 3) {
            b.append('|');
         }
      }
      return b.toString();
   }

   /** Current choice of 4 columns, for the values line under the knobs. */
   private String choices(final int from) {
      final StringBuilder b = new StringBuilder();
      for (int i = from; i < from + 4; i++) {
         final String v = columnExists[i] ? selections[i] : "";
         b.append(v.length() > 11 ? v.substring(0, 11) : v);
         if (i < from + 3) {
            b.append('|');
         }
      }
      return b.toString();
   }

   private void updateDisplay() {
      if (!isActive()) {
         return;
      }
      sendToDisplay(0, DisplayUtil.padString("BROWSER " + lastKnob, 20) + "|<AUDIT>");
      if (touchedIndex >= 0 && touchedIndex < 8) {
         sendToDisplay(1, columnNames[touchedIndex] + ": " + selections[touchedIndex]);
      } else {
         sendToDisplay(1, "> " + selections[RESULTS]);
      }
      sendToDisplay(2, names(0));
      sendToDisplay(3, names(4));
      getDriver().sendValuesRow(0, choices(0));
      getDriver().sendValuesRow(1, choices(4));
   }

   @Override
   protected void notifyEncoderTouched(final int index, final boolean v) {
      if (!isActive()) {
         return;
      }
      if (v) {
         touchedIndex = index;
      } else if (touchedIndex == index) {
         touchedIndex = -1;
      }
      updateDisplay();
   }

   @Override
   protected void doNotifyMainTouched(final boolean touched) {
   }

   @Override
   protected void doNotifyMacroDown(final boolean active) {
   }

   @Override
   protected void doDeactivate() {
      super.doDeactivate();
      clearDisplay();
   }

   @Override
   protected void doActivate() {
      super.doActivate();
      setKnobSensitivity(1.0);
      lastKnob = "";
      updateDisplay();
   }

   @Override
   public void jogWheelAction(final int increment) {
      if (increment > 0) {
         cursorResult.selectNext();
      } else {
         cursorResult.selectPrevious();
      }
   }

   @Override
   public void jogWheelPush(final boolean push) {
      if (push) {
         browser.commit();
      }
   }
}
