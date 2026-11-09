# Contributing

Thanks for your interest. ianox is pre-alpha, so the best contributions right now are bug
reports with steps to reproduce, and small focused fixes.

## Setup

You need Rust 1.77 or newer, Node.js 20.19+ (or 22.12+) and the system libraries Tauri
requires on Linux (see the README).

```sh
npm install
npm run dev:app        # the app with hot reload
```

## Before you open a pull request

```sh
npm run check          # type-check the frontend
npm run i18n:check     # every visible string has an English and a Spanish translation
cd src-tauri && cargo test
```

## Conventions

- Visible text goes through `t("...")` and is translated in `locales/en.po` and
  `locales/es.po`; run `npm run i18n` to regenerate the catalogues. Code and comments are in
  English.
- Keep changes small and say what you verified. If something could not be tested, say so.
- Add a test when you fix a bug in the Rust backend or in the pure TypeScript helpers.
- Do not copy code from other projects unless its license allows it; if it does, keep its
  notice and add it to `THIRD_PARTY_NOTICES.md`.

## Licensing

By contributing you agree that your work is released under the [MIT License](LICENSE).

Report problems at <https://github.com/Subdes/ianox/issues>.
