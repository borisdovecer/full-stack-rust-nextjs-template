# web/

The Next.js UI. It has no server logic: a page only sets its title and renders a client view, and all data comes from the Rust API in `services/api`, in the browser.

- How it is built: `docs/system/web.md`
- The rules for working on it: `docs/rules/ui.md` and `web/CLAUDE.md`
- Run it with the API: `scripts/dev.sh` from the repository root
- `AGENTS.md` is written by `next dev` (Next's notes for coding agents); `CLAUDE.md` includes it. Keep it committed, or `next dev` re-creates it as a change

```bash
npm run dev          # http://127.0.0.1:3000, /api/* forwarded to API_URL
npm run api:types    # after an API change
npm run lint && npm run typecheck && npm test
```
