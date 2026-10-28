# NASpresso

- `server/` — Rust server on axum (`cargo run`, listens on `HOST`:`PORT`, both required, e.g. in `server/.env`)
- `client/` — Vite + TypeScript (`npm run dev -w client`)

## Setup

```sh
npm install   # installs deps and the prek pre-commit and pre-push hooks
```

If your npm config has `ignore-scripts=true`, run `npm run postinstall && npm run prepare` afterwards: the first applies `client/patches` (antd needs a fix for `@faiwer/react`), the second installs the hooks.

The pre-commit hook runs on staged files only:

- client: `oxfmt`, `oxlint`
- server: `cargo fmt`

The pre-push hook runs `cargo clippy -D warnings` when the pushed commits touch Rust files.

Whole-project checks also run on demand, since partially staged commits may not compile:

- `npm run typecheck` — `tsrs --noEmit --fast` for the client
- `npm run clippy` — `cargo clippy -D warnings` for the server

Run all hooks on the whole repo with `npm run lint`.
