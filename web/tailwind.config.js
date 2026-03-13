/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'dac-bg': '#0f1117',
        'dac-surface': '#1a1d27',
        'dac-border': '#2d3148',
        'dac-text': '#e2e8f0',
        'dac-muted': '#8892a4',
      },
    },
  },
  plugins: [],
}
