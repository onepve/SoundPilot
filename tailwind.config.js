/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{vue,ts}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        mint: {
          50: '#f0fdf9',
          100: '#dcfce9',
          200: '#bbf7d4',
          300: '#86efb0',
          400: '#4cd695',
          500: '#22b573',
          600: '#15803d',
          700: '#166534',
          800: '#14532d',
          900: '#052e16',
        },
        teal: {
          450: '#0d9488',
        },
      },
      fontFamily: {
        sans: [
          '"Segoe UI"',
          'system-ui',
          '-apple-system',
          '"PingFang SC"',
          '"Microsoft YaHei UI"',
          '"Microsoft YaHei"',
          'sans-serif',
        ],
      },
      boxShadow: {
        card: '0 1px 2px rgba(16,24,40,.05), 0 1px 3px rgba(16,24,40,.08)',
        'card-hover': '0 4px 6px -1px rgba(16,24,40,.08), 0 10px 24px -6px rgba(16,24,40,.12)',
        pop: '0 12px 40px -8px rgba(16,24,40,.22), 0 4px 12px rgba(16,40,30,.08)',
      },
    },
  },
  plugins: [],
}
