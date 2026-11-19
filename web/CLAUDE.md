@AGENTS.md

# web/: Claude Code briefing

Read the root `CLAUDE.md` first. The UI rules are in `docs/rules/ui.md`; how the web is built is in `docs/system/web.md`. This file is the short version.

## Stack

Node 26 (`nvm use`), Next.js 16 App Router with the React Compiler, TypeScript 5 strict (`noUncheckedIndexedAccess`), typed routes, Tailwind v4, shadcn on Base UI (`base-nova`), TanStack Query, zod, next-themes, sonner, vitest + Testing Library, oxlint + oxfmt, npm.

## Must

- No server logic: no route handlers, no server actions, no `cookies()`, no secrets. All data comes from the Rust API through `src/lib/api/client.ts`.
- `src/lib/api/schema.d.ts` is generated (`npm run api:types`). Never edit it.
- Errors: render `describeError(e).words`; decide on `.code`. Words live only in `src/lib/errors.ts`.
- shadcn primitives from `src/components/ui/` (add with `npx shadcn@latest add <name>`); compose with `render={...}`, not `asChild`. A `Button` rendering a `Link` needs `nativeButton={false}`.
- After adding or regenerating a primitive: point `cn` at `@/lib/utils`, and rewrite `data-horizontal:` / `data-vertical:` variants to `data-[orientation=…]:` (Base UI sets `data-orientation`). Tests catch both.
- Our components: `src/components/<Name>/<Name>.tsx`, listed in `REUSABLE` (and shown on `/components`) or `VIEWS` in `ComponentsShowcase.tsx`.
- Theme tokens only, no raw colours; a new token goes into both themes in `globals.css`.
- Typed routes: an `href` must be a real route; cast a built path with `as Route`.
- `npm run lint && npm run typecheck && npm test` before calling work done.

## Tests

Beside the file (`Name.test.tsx`), vitest in jsdom with the React Compiler. Import `describe/it/expect` from `vitest`. For components that call the API, use `renderWithClient` and `mockApi` from `src/test/render.tsx`.
