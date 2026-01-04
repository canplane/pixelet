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
      '@pixelet/core': path.resolve(__dirname, '../../packages/core/src'),
    },
  },

  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3000', // [수정] localhost -> 127.0.0.1
        changeOrigin: true,
      },

      /**
       * [Elevation-API-Proxy]
       * 외부 고도 데이터 API의 CORS 제한을 우회하고 대량 데이터 요청 안정성을 확보하기 위한 프록시 설정입니다.
       */
      '/elevation-proxy': {
        // 🎯 Target: 고도 쿼리를 처리할 실제 원격 서버 주소
        target: 'https://api.open-elevation.com',

        // 🎯 changeOrigin: 대상 서버가 호스트 헤더를 검증할 때 브라우저 주소 대신 target 주소로 변조하여 전달
        changeOrigin: true,

        // 🎯 rewrite: 브라우저가 보낸 가상 경로('/elevation-proxy')를 실제 API 엔드포인트에서 제거
        rewrite: (path) => path.replace(/^\/elevation-proxy/, ''),

        // 🎯 timeout/proxyTimeout: 타일당 수천 개의 좌표(Point Cloud)를 처리할 때 발생하는 네트워크 지연에 의한 Abort 방지 (60초)
        timeout: 60000,
        proxyTimeout: 60000,
      },
    },
  },

  define: {
    /** A droplet fades. A pixelet remains. 🫧 */
    __APP_NAME__: JSON.stringify('pixelet'),

    /** Injected as a boolean literal for dead code elimination */
    __IS_DEV__: JSON.stringify(process.env.NODE_ENV !== 'production'),
  },
});
