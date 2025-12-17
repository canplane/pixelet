import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';

export default [
  /* ---------------------------------------------
   * Base JS rules
   * --------------------------------------------- */
  js.configs.recommended,

  /* ---------------------------------------------
   * TypeScript (apps/*)
   * - project: false for performance
   * - unused vars are warnings (not errors)
   * --------------------------------------------- */
  {
    files: ['apps/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        project: false, // 🔥 performance-first
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    plugins: {
      '@typescript-eslint': tseslint.plugin,
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      ...prettier.rules,

      /* 🔹 soften unused vars */
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': [
        'warn',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],

      /* 🔹 enforce shared package boundary */
      'no-restricted-imports': [
        'error',
        {
          patterns: ['@pixelet/shared/src/*'],
        },
      ],
    },
  },

  /* ---------------------------------------------
   * React (apps/web only)
   * --------------------------------------------- */
  {
    files: ['apps/web/**/*.{ts,tsx}'],
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
   * API (apps/api only)
   * --------------------------------------------- */
  {
    files: ['apps/api/**/*.ts'],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
    rules: {
      'no-console': 'off', // server logs are fine
    },
  },

  /* ---------------------------------------------
   * Ignore
   * --------------------------------------------- */
  {
    ignores: ['**/node_modules/**', '**/dist/**'],
  },
];
