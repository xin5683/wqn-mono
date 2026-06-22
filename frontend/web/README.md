# Wrong Question Notebook Web

Next.js frontend for Wrong Question Notebook.

The browser talks to `/api/*`; Next rewrites those requests to the Rust API
configured by `WQN_API_BASE_URL`. Authentication uses the Rust API's local
HttpOnly session cookie.

## Local development

```bash
npm install
cp env.example .env.local
npm run dev
```

Set `WQN_API_BASE_URL` to the running Rust backend, for example
`http://127.0.0.1:8080`.
