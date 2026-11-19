# UI rules

Every page and component in `web/`. How the web is built: `docs/system/web.md`.

## Components

- shadcn first: a control shadcn has (button, input, select, dialog, ...) is that component from `src/components/ui/`. No hand-made twin. Missing one: `npx shadcn@latest add <name>`.
- Style primitives through props and `className`; do not rewrite their internals.
- Our components live in `src/components/<Name>/<Name>.tsx`, PascalCase, one folder each, tests beside them. A page in `src/app/` only imports its view.
- A reusable component is added to `REUSABLE` and shown on `/components`; a one-off view goes into `VIEWS`. The test fails until it is in one of them.
- Composition in Base UI uses `render={<Link href="..." />}`, not `asChild`. A `Button` that renders a link also takes `nativeButton={false}`, or Base UI warns that the button lost its semantics.
- Base UI marks orientation as `data-orientation="horizontal"`. The shadcn generator writes `data-horizontal:` variants, which never match; `src/components/ui/` uses `data-[orientation=horizontal]:` instead, and a test fails if a regenerated file brings the old form back.

## Theme and styling

- Tailwind utilities only, merged with `cn()`. No CSS files besides `globals.css`.
- Colours and radii come from theme tokens (`bg-background`, `text-muted-foreground`, `rounded-lg`). No hex, rgb or oklch values in components; a test fails on them.
- A new token goes into both theme blocks in `globals.css`. A new text-on-surface pair goes into the contrast test, which requires 4.5:1 in both themes.
- Type scale: `text-xs` for labels, hints and table cells, `text-sm` for body (the default), `text-base` and `text-lg` for headings, `text-2xl` for a page title. No arbitrary sizes like `text-[13px]`.
- Weights: `font-medium` and `font-semibold`.

## Layout

- Works from a 390 px phone up. Check 390, 768 and 1440 px before calling visual work done.
- The page scrolls, never a box inside it. Only a table, a code block or a tab row scrolls sideways, inside its own box.
- A page body is `mx-auto w-full max-w-<n> px-4 md:px-5 py-4`. Grids fall back to one column.
- Sidebar from `md` up, bottom nav below it; both are drawn and one is hidden with a width class. No screen-size checks in JavaScript.
- Loading is a `Skeleton` at the final size. Empty is `Empty`, never a bare "none". Failure is `ErrorAlert`.

## Words

- One sentence under a heading. Two only when the second saves a support question.
- Say what to do, not how it works.
- Buttons and labels start with a capital letter and stay sentence case: "Create user", not "Create User".
- Error words live only in `src/lib/errors.ts`. Components render `describeError(e).words`, never the API's `message`, and decide on `code`, never on text.
- An error says what is wrong and what to do: "Wrong email or password.", "Too many attempts. Try again after 14:05."
- Numbers and dates go through `src/lib/format.ts`. An unknown value shows as "—", never as 0.

## Forms

- Check in the browser first with the API's rules (`src/lib/validation.ts`), then send. Show each problem under its field.
- `useForm` handles both sides: browser checks and the API's `details.fields` land in the same place.
- A destructive or access-removing action asks first with `ConfirmModal`.
- Disable the submit button while sending and show a `Spinner` in it.

## Accessibility

- Every control has a label (`Field` wires `htmlFor`, `aria-invalid`, `aria-describedby`).
- Icon-only buttons have an `aria-label`.
- No `autoFocus`; no information only in colour (badges carry words).
- Tooltips never hold something a person needs to act: they do not open on touch.

## Code

- TypeScript strict: no `any`, no non-null `!` without a comment saying why it is safe. Index access is `T | undefined`.
- The site sends a Content-Security-Policy (`deploy/Caddyfile`): same origin only, plus browser Sentry. A new outside host (fonts, analytics, an RPC, an image host) is added there in the same change, or the browser blocks it.
- Server data goes through TanStack Query and the generated `api` client. No `fetch` to the API by hand, except for non-JSON answers (the help pages).
- `npm run lint && npm run typecheck && npm test` before calling work done.
