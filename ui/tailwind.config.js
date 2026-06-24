/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Driven by CSS variables set from the active theme (see index.css).
        nova: {
          bg: "var(--nova-bg)",
          fg: "var(--nova-fg)",
          accent: "var(--nova-accent)",
          border: "var(--nova-border)",
          tabActive: "var(--nova-tab-active)",
          tabInactive: "var(--nova-tab-inactive)",
        },
      },
      fontFamily: {
        mono: ["var(--nova-font)", "Cascadia Code", "Consolas", "monospace"],
      },
      keyframes: {
        "fade-in": { from: { opacity: "0", transform: "translateY(4px)" }, to: { opacity: "1", transform: "translateY(0)" } },
        "scale-in": { from: { opacity: "0", transform: "scale(0.98)" }, to: { opacity: "1", transform: "scale(1)" } },
      },
      animation: {
        "fade-in": "fade-in 0.15s ease-out",
        "scale-in": "scale-in 0.12s ease-out",
      },
    },
  },
  plugins: [],
};
