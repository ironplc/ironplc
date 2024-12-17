import typescriptEslint from "@typescript-eslint/eslint-plugin";
import stylistic from '@stylistic/eslint-plugin'
import tsParser from "@typescript-eslint/parser";

export default [
    {
        files: [
            "**/*.ts"
        ]
    },
    {
        ignores: ["**/out", "**/dist", "**/*.d.ts"],
    },
    stylistic.configs.customize({
        // the following options are the default values
        indent: 2,
        quotes: 'single',
        semi: true,
        jsx: true
    }),
    {
        plugins: {
            "@typescript-eslint": typescriptEslint,
        },

        languageOptions: {
            parser: tsParser,
            ecmaVersion: 6,
            sourceType: "module",
        },

        rules: {
            "@typescript-eslint/naming-convention": "warn",
            curly: "warn",
            eqeqeq: "warn",
            "no-throw-literal": "warn",
        },
    }
];