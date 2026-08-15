import { definePreset } from "@primeuix/themes";
import Aura from "@primeuix/themes/aura";

export const AdminTheme = definePreset(Aura, {
  semantic: {
    primary: {
      50: "#f4f4f5",
      100: "#e4e4e7",
      200: "#d4d4d8",
      300: "#a1a1aa",
      400: "#71717a",
      500: "#3f3f46",
      600: "#27272a",
      700: "#18181b",
      800: "#18181b",
      900: "#09090b",
      950: "#09090b",
    },
    colorScheme: {
      light: {
        surface: {
          0: "#ffffff",
          50: "#fafafa",
          100: "#f4f4f5",
          200: "#e4e4e7",
          300: "#d4d4d8",
          400: "#a1a1aa",
          500: "#71717a",
          600: "#52525b",
          700: "#3f3f46",
          800: "#27272a",
          900: "#18181b",
          950: "#09090b",
        },
      },
    },
  },
});
