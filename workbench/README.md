# Optimist Workbench

The Vue workbench is the interactive client for Optimist's typed systems model. It currently supports project selection and creation, typed node creation, `part_of` and `requires` relationship creation, graph search/kind filters, Cytoscape navigation, and a typed read-only inspector.

## Run locally

Start the Optimist API from the repository root:

```sh
cargo run -- server --bind 127.0.0.1:3000
```

Then start Vite:

```sh
cd workbench
npm install
npm run dev
```

Vite proxies `/api` to `http://127.0.0.1:3000`. Set `OPTIMIST_API_URL` before `npm run dev` to use another server.

## Validate

```sh
npm test
npm run build
npm run test:e2e -- --workers=1
npm run test:e2e:real
npm audit
```

`test:e2e` uses deterministic mocked API state for desktop/mobile layout, screenshots, canvas-pixel checks, and a 100-node render bound. `test:e2e:real` starts Axum and Vite on isolated ports and verifies project, node, and relationship commands through the real proxy. Both Playwright configurations use a non-interactive line reporter and exit after completion.
