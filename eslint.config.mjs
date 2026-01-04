import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';

export default [
  {
    ignores: ['**/node_modules/**', '**/dist/**', '**/build/**', '**/temp/**'],
  },

  js.configs.recommended,

  /* ---------------------------------------------
   * 1. Base TypeScript Rules (All Packages)
   * --------------------------------------------- */
  {
    // shared와 모든 app을 포함
    files: ['packages/shared/**/*.{ts,tsx}', 'apps/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        project: false,
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
    },
    plugins: {
      '@typescript-eslint': tseslint.plugin,
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      ...prettier.rules,

      '@typescript-eslint/explicit-module-boundary-types': 'warn',
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': [
        'warn',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],
      'no-restricted-imports': [
        'error',
        { patterns: ['@pixelet/shared/src/*'] },
      ],
    },
  },

  /* ---------------------------------------------
   * 2. Shared Engine Specific (Pure Logic)
   * 브라우저/노드 어디서든 돌아가야 하므로 환경을 제한합니다.
   * --------------------------------------------- */
  {
    files: ['packages/shared/**/*.ts'],
    languageOptions: {
      globals: {
        // 엔진 코드는 환경 의존성을 최소화해야 하므로 기본 전역 변수만 허용
        ...globals.es2021,
      },
    },
    rules: {
      // 엔진급 코드에서는 any 사용을 더 엄격히 제한하는 것을 권장
      '@typescript-eslint/no-explicit-any': 'warn',
    },
  },

  /* ---------------------------------------------
   * 3. Web Specific (React/Browser)
   * --------------------------------------------- */
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
      'react-refresh/only-export-components': [
        'warn',
        { allowConstantExport: true },
      ],
    },
  },

  /* ---------------------------------------------
   * 4. API Specific (Node/Server)
   * --------------------------------------------- */
  {
    files: ['apps/api/**/*.ts'],
    languageOptions: {
      globals: { ...globals.node },
    },
    rules: {
      'no-console': 'off',
    },
  },
];
