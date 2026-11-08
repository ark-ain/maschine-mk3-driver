package com.bitwig.extensions.controllers.nativeinstruments.maschinelinux;

import com.bitwig.extension.api.PlatformType;
import com.bitwig.extension.controller.AutoDetectionMidiPortNamesList;
import com.bitwig.extension.controller.ControllerExtensionDefinition;
import com.bitwig.extension.controller.api.ControllerHost;

import java.util.UUID;

public class MaschineMk3ExtensionDefinition extends ControllerExtensionDefinition {
   private static final UUID DRIVER_ID = UUID.fromString("6f1c2a4e-8b3d-4c7a-9e51-3d2b7a0c9f14");

   public MaschineMk3ExtensionDefinition() {
   }

   @Override
   public String getName() {
      return "Maschine Mk3 (Linux)";
   }

   @Override
   public String getAuthor() {
      return "Bitwig (Linux port)";
   }

   @Override
   public String getVersion() {
      return "1.0";
   }

   @Override
   public UUID getId() {
      return DRIVER_ID;
   }

   @Override
   public String getHardwareVendor() {
      return "Native Instruments";
   }

   @Override
   public String getHardwareModel() {
      return "Maschine Mk3";
   }

   @Override
   public int getRequiredAPIVersion() {
      return 18;
   }

   @Override
   public int getNumMidiInPorts() {
      return 1;
   }

   @Override
   public int getNumMidiOutPorts() {
      return 1;
   }

   @Override
   public void listAutoDetectionMidiPortNames(final AutoDetectionMidiPortNamesList list,
                                              final PlatformType platformType) {
      if (platformType == PlatformType.WINDOWS) {
         list.add(new String[]{"Maschine MK3 Ctrl MIDI"}, new String[]{"Maschine MK3 Ctrl MIDI"});
      } else if (platformType == PlatformType.MAC) {
         list.add(new String[]{"Maschine MK3 Virtual Input"}, new String[]{"Maschine MK3 Virtual Output"});
      } else if (platformType == PlatformType.LINUX) {
         list.add(new String[]{"Maschine MK3 Ctrl MIDI"}, new String[]{"Maschine MK3 Ctrl MIDI"});
         list.add(new String[]{"Virtual Raw MIDI/10"}, new String[]{"Virtual Raw MIDI/10"});
      }
   }

   @Override
   public String getHelpFilePath() {
      return "Controllers/Native Instruments/Maschine MK3 Plus/Maschine MK3 Plus.pdf";
   }

   @Override
   public String getSupportFolderPath() {
      return "Controllers/Native Instruments/Maschine MK3 Plus";
   }

   @Override
   public MaschineExtension createInstance(final ControllerHost host) {
      return new MaschineExtension(this, host, MaschineMode.MK3);
   }
}
