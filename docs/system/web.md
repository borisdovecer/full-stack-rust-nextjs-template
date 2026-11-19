# Web app

Last checked against the code on 2026-11-19.

The UI in `web/`: Next.js 16 (App Router), React 19 with the React Compiler, TypeScript strict, Tailwind v4, shadcn on Base UI, TanStack Query. It has no server logic: a page only sets its title and renders a client view. Every page builds static except `/help/[slug]` and `/users/[id]`, which Next renders on request to read the id from the path and nothing else. All data comes from the Rust API, in the browser.

## Pages

| Route | Who | Shows |
|---|---|---|
| `/` | anyone | welcome, Log in and Register (or Open the dashboard when signed in) |
| `/login` | anyone | a method switch when the server offers more than one (`login_methods`), then that method's fields (email and password; email, "Send code", then the code; email, "Connect a wallet", then sign; or "Continue with Google / GitHub"), and the reason when a provider sent the person back with `?error=`; returns to `?next=` after sign-in |
| `/register` | anyone | the same method switch, first name, last name, email, the method's fields, optional bio; with Google or GitHub, the provider first and then `/register?via=external` asks for the names; a note when registration is closed |
| `/pending` | pending users | the waiting page |
| `/dashboard` | signed in | the user's summary; with `users.read`, the user list with search, filters, sorting and paging; with `users.manage`, Create user |
| `/users/[id]` | `users.read` | account facts and activity; with `users.manage`, first and last name, bio, role, status, permissions, set a password, switch to an email code when the server offers it |
| `/settings` | signed in | theme, profile (first name, last name, bio), sign-in method (change the password, or switch to another offered method: a password, email codes after a code to the account's address, a wallet that signs, or Google or GitHub with the same email), sessions, log out everywhere |
| `/help`, `/help/[slug]` | signed in | the system docs from `GET /api/help` |
| `/components` | development only | every reusable component in its states |

Route groups: `src/app/(public)/` for signed-out pages, `src/app/(app)/` for the signed-in shell (`AppShell` + `AuthGate`).

## Data

- `src/lib/api/schema.d.ts` is generated from the API's OpenAPI document by `npm run api:types`. Never edit it.
- `src/lib/api/client.ts` exports `api` (typed `openapi-fetch` client, same origin, JSON on every request) and `call()`, which returns the data or throws an `ApiError` with the API's `code`.
- Server data lives in TanStack Query. Keys are in `src/lib/queries.ts`. A `401` from any call re-checks `/api/me`, and `AuthGate` sends the person to `/login`.
- There is no client state library: component state is enough so far.

## Access

`AuthGate` reads `GET /api/me` and decides with the pure `gate()` function: signed out → `/login?next=…`, pending → `/pending`, missing permission → a "cannot see this page" view. It is navigation only; the API checks every request.

## Errors

`src/lib/errors.ts` holds the words for every API error code and every field problem. Components render `describeError(e).words` and never the API's `message`. `ErrorAlert` shows the words and the request id. A form's field problems, from the browser's own check (`src/lib/validation.ts`, the API's rules in zod) or from the API, land under their fields through `useForm`.

## Theme

Tokens in `src/app/globals.css`: one list, light values on `:root`, dark on `.dark`. `next-themes` sets the class before first paint (no flash) and remembers the choice in the browser. Signed in, the choice is also saved to the account and `ThemeSync` applies it on other browsers. `globals.css.test.ts` checks every text/surface pair at 4.5:1 contrast in both themes and that both themes define the same tokens.

## Components

- `src/components/ui/`: shadcn primitives as the CLI writes them. Add one with `npx shadcn@latest add <name>`.
- `src/components/<Name>/<Name>.tsx`: ours, one folder each, with tests beside them. Reusable ones are listed in `REUSABLE` in `ComponentsShowcase.tsx` and shown on `/components`; page views are listed in `VIEWS`. A test fails on a folder in neither list.
- Wallets: `WalletConnect` wraps `@solana/wallet-adapter-react` (wallet-standard detection, no adapters listed, no RPC connection) with our own picker dialog instead of the adapter's UI kit and its stylesheet. Pages load it with `next/dynamic` only when the wallet is chosen, so the wallet libraries stay out of every other page.

## Sentry

`src/instrumentation-client.ts` starts `@sentry/react` when `NEXT_PUBLIC_SENTRY_DSN` is set: errors only, no replay, no tracing, no user data, cookies and bodies stripped.

## Commands

| Command | Does |
|---|---|
| `npm run dev` | dev server on `WEB_PORT` (3000), `/api/*` forwarded to `API_URL` |
| `npm run build`, `npm start` | production build and server |
| `npm run api:types` | regenerates `schema.d.ts` from the API |
| `npm run lint`, `typecheck`, `test`, `format` | oxlint, tsc, vitest, oxfmt |
