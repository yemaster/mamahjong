import { definePreset } from "@primeuix/themes";
import Aura from "@primeuix/themes/aura";

export const AdminTheme = definePreset(Aura, {
  semantic: {
    primary: {
      50: "{emerald.50}",
      100: "{emerald.100}",
      200: "{emerald.200}",
      300: "{emerald.300}",
      400: "{emerald.400}",
      500: "{emerald.600}",
      600: "{emerald.700}",
      700: "{emerald.800}",
      800: "{emerald.900}",
      900: "{emerald.950}",
      950: "#022c22",
    },
    colorScheme: {
      light: {
        surface: {
          0: "#ffffff",
          50: "#f8faf9",
          100: "#f1f4f2",
          200: "#e3e8e5",
          300: "#cbd3ce",
          400: "#9ca8a1",
          500: "#6f7c74",
          600: "#4f5b54",
          700: "#38423c",
          800: "#242b27",
          900: "#171c19",
          950: "#0c0f0d",
        },
      },
    },
  },
});
