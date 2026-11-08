package com.bitwig.extensions.util;

public class ColorUtils
{
   public static float[] rgbToHsb(final int red, final int green, final int blue, final float[] hsb)
   {
      final int max = Math.max(red, Math.max(green, blue));
      final int min = Math.min(red, Math.min(green, blue));
      final float saturation = max == 0 ? 0 : (max - min) / (float)max;
      float hue = 0;

      if (saturation != 0)
      {
         final float range = max - min;
         final float r = (max - red) / range;
         final float g = (max - green) / range;
         final float b = (max - blue) / range;

         if (red == max)
            hue = b - g;
         else if (green == max)
            hue = 2.0f + r - b;
         else
            hue = 4.0f + g - r;

         hue = hue / 6.0f;

         if (hue < 0)
            hue = hue + 1.0f;
      }

      hsb[0] = hue;
      hsb[1] = saturation;
      hsb[2] = max / 255.0f;
      return hsb;
   }
}
