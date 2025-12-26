import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

import tailwindcss from '@tailwindcss/vite';
import svgr from 'vite-plugin-svgr';

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(), // Tailwind
    svgr({ svgrOptions: { titleProp: true } }), // SVGR
  ],

  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
      '@pixelet/shared': path.resolve(__dirname, '../../packages/shared/src'),
    },
  },

  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3000', // [수정] localhost -> 127.0.0.1
        changeOrigin: true,
        secure: false,
      },
    },

    open: true, // 서버 시작 시 자동으로 브라우저 오픈
  },

  define: {
    /** A droplet fades. A pixelet remains. 🫧 */
    __APP_NAME__: JSON.stringify('pixelet'),

    /** Injected as a boolean literal for dead code elimination */
    __IS_DEV__: JSON.stringify(process.env.NODE_ENV !== 'production'),
  },
});
