# Development rules

How a bug or a missing feature goes from the first note to finished work. This file is the overview; the other rule files cover their own subject and win where they disagree with it.

## Roles

| Role | Does |
|---|---|
| Owner | decides what is built, answers questions, approves proposals. On a solo project, you |
| Developer | writes documents and proposals, implements, tests, closes the work |
| Agent (Claude Code) | may write any part of a document and implement approved work. Never approves, never answers a question for the owner, never checks the result for the owner, never commits unless asked |

## One document per piece of work

Every change starts as one document:

- something exists but does not work as it should: a **bug report** in `docs/bug_report/`, from `bug_report_template.md`
- something is missing: a **feature request** in `docs/feature_request/`, from `feature_request_template.md`
- not sure which: a bug report with priority `info`

That document is the only record of the work. The facts, the proposal, the questions, the owner's answers, the approval and what was implemented are all written into it. File name: `DD-MM-YYYY-priority-category-short-title.md`, for example `18-11-2026-high-api-login-throttle-counts-successes.md`. One problem per document.

## The workflow

| # | Step | Who | Status after |
|---|---|---|---|
| 1 | Write the document from its template | anyone | `draft` |
| 2 | Verify it in the code and the data | developer or agent | `confirmed` or `discarded` |
| 3 | Write the proposal: what changes, where, why, how it is tested | developer or agent | `confirmed` |
| 4 | Ask what only the owner can decide, each question with options and a recommendation | developer or agent | `waiting-decision` |
| 5 | Answer and approve, written into the document | owner | `approved` |
| 6 | Write the Tasks: a checklist of the approved work, one line per piece | developer or agent | `approved` |
| 7 | Implement task by task, ticking each, with tests, until `scripts/check.sh` passes | developer or agent | `in-progress` |
| 8 | Fill the Implementation section, update `docs/system/`, and ask the owner to check the result | developer or agent | `review` |
| 9 | Check that the bug is fixed or the feature works, written into the document | owner | `review` |
| 10 | Move the document to `docs/archived/<same folder>/` | developer or agent | `done` |

No code before step 6. Steps 1 to 4 may happen in one sitting, and the owner may answer and approve in one message. An answer, approval or check that is not written in the document does not count. If the owner finds a problem at step 9, it goes back to step 7 with a new task, or, if it is a different problem, into a new document.

A typo in a comment, a log line or a doc does not need a document.

## Building

- Read the matching page in `docs/system/` before changing a part of the system. When the page and the code disagree, the code wins and the page is fixed in the same change.
- A new language, framework, database or paid service is the owner's decision, asked before it is used. So is ignoring a security advisory: it goes into `deny.toml` with its reason, or the CI's `npm audit` threshold, only with the owner's approval.
- Every behaviour change comes with a test: Rust routes with `#[sqlx::test]` in `services/api/tests/`, web logic and components with vitest beside the file.
- A new API route or DTO: run `npm run api:types` in `web/` and keep the regenerated file.
- Services, crates and their layout follow `architecture.md`; database changes follow `database.md`; UI changes follow `ui.md`; commits follow `git.md`.
- Never write a secret into code, docs, tests, logs or a commit. Secrets live in `.env` files, which are gitignored.
- A code comment says only what the code cannot: a rule a reader must not break, a unit, why the obvious simpler version is wrong. No names, no dates, no history; those belong in the document.

## Done means

- `scripts/check.sh` passes: format, clippy with warnings as errors, Rust tests, the OpenAPI types check, lint, typecheck, web tests, build.
- Visual changes were looked at in a browser at 390 px, 768 px and 1440 px, in both themes.
- `docs/system/` describes the new behaviour.
- Every task in the document is ticked, and the Implementation section says what changed, where, and how it was tested.
- The owner checked the result, written in the document.
