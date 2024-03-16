const { fontFamily } = require('tailwindcss/defaultTheme');

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./templates/*.html'],
  theme: {
    extend: {
      fontFamily: {
        sans: ['InterVariable', ...fontFamily.sans],
      },
    },
  },
  plugins: [require('daisyui')],
  daisyui: {
    themes: ['sunset', 'wireframe'],
    darkTheme: 'sunset',
  },
}

