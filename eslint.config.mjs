import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import prettierConfig from 'eslint-config-prettier';

export default tseslint.config(
  // ---------------------------------------------------------------------------
  // 1. Global Ignores (전역 무시 설정)
  // ---------------------------------------------------------------------------
  {
    ignores: ['**/node_modules/**', '**/dist/**', '**/build/**', '**/temp/**'],
  },

  // ---------------------------------------------------------------------------
  // 2. Base Configurations (JS + TS 권장 설정 상속)
  // ---------------------------------------------------------------------------
  js.configs.recommended,
  ...tseslint.configs.recommended,

  // ---------------------------------------------------------------------------
  // 3. Shared TypeScript Rules (All Packages)
  // 모든 패키지에 적용되는 그라운드 룰입니다.
  // ---------------------------------------------------------------------------
  {
    files: ['**/*.{ts,tsx}'], // 범위를 전체로 확장
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
      parserOptions: { project: false },
    },
    rules: {
      // 불필요한 줄바꿈을 유발하는 스타일 규칙 제거 및 실용적 룰셋 적용
      '@typescript-eslint/explicit-module-boundary-types': 'off', // 필요할 때만 켜기
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      '@typescript-eslint/no-explicit-any': 'warn',

      // 모노레포 내부 참조 규칙 (순환 참조 방지 등)
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['@pixelet/core/src/*'],
              message: 'Use barrel files (index.ts) instead of deep imports.',
            },
          ],
        },
      ],
    },
  },

  // ---------------------------------------------------------------------------
  // 4. Domain Specific: Core Engine (Pure TS)
  // ---------------------------------------------------------------------------
  {
    files: ['packages/core/**/*.ts'],
    languageOptions: {
      globals: { ...globals.es2021 }, // 브라우저/노드 종속성 배제
    },
    rules: {
      // 엔진은 더 엄격하게 관리
      'no-console': ['warn', { allow: ['warn', 'error'] }],
      '@typescript-eslint/explicit-member-accessibility': ['warn', { accessibility: 'no-public' }],
    },
  },

  // ---------------------------------------------------------------------------
  // 5. Domain Specific: Web Apps (React)
  // ---------------------------------------------------------------------------
  {
    files: ['apps/web/**/*.{ts,tsx}'],
    languageOptions: {
      globals: { ...globals.browser },
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
    },
  },

  // ---------------------------------------------------------------------------
  // 6. Domain Specific: API Server (Node)
  // ---------------------------------------------------------------------------
  {
    files: ['apps/api/**/*.ts'],
    languageOptions: {
      globals: { ...globals.node },
    },
    rules: {
      'no-console': 'off', // 서버 로그 허용
    },
  },

  // ---------------------------------------------------------------------------
  // 7. Prettier Override (Always Last)
  // 모든 스타일 관련 규칙을 Prettier가 덮어씌웁니다.
  // ---------------------------------------------------------------------------
  prettierConfig,
);
