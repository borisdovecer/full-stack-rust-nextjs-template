---
name: app-api
description: Use the running app through its HTTP API as a real account: check health, read who is signed in, list, search and read users, create users, change a user's name, bio, role, status, permissions or password, end sessions, and read the in-app help pages. Use when the user asks to look at or change users or accounts in the app, check whether the API or database is up, try a change against the running app, or read the app's system docs as served. Needs the API running (scripts/dev.sh) and the first admin's sign-in in .env (written by scripts/setup.sh).
---

# app-api

Calls the app's Rust API as the first admin. Every call goes through `scripts/api.mjs`, which signs in when needed and keeps the session cookie in `~/.config/app/<project>-<hash>/cookie` (mode 600), a folder of its own for each checkout.

## Account

The script reads the project's `.env`:

- `ADMIN_EMAIL` and `ADMIN_PASSWORD`: written by `scripts/setup.sh` when it creates the first admin. For a database whose admin existed before, the person fills them in by hand.
- `API_BIND` gives the URL (`http://127.0.0.1:8080` by default). `APP_API_URL` in the environment overrides it.

If they are missing, the script says so; ask the person to fill them in. Never read or print `.env`, never ask for the password in the chat, never put the password in a command.

## Calling the API

```bash
node skills/app-api/scripts/api.mjs METHOD PATH [JSON_BODY]
```

It prints the status line and the JSON answer, and exits 0 on success, 1 on an API error, 2 when it cannot run.

The routes, with their parameters and answers, are in the OpenAPI document. Read it when unsure:

```bash
node skills/app-api/scripts/api.mjs GET /api/openapi.json
```

(In production the document is for admins only.)

Common calls:

| Task | Call |
|---|---|
| Is it up? | `GET /api/health` |
| Who am I, what may I do | `GET /api/me` |
| Find users | `GET /api/users?search=ana&status=active&sort=name_asc&page=1&per_page=20` |
| One user, with recent activity | `GET /api/users/<id>` |
| Create a user | `POST /api/users '{"email":"...","first_name":"...","last_name":"...","bio":"...","password":"...","role":"user"}'` |
| Approve a pending user | `PATCH /api/users/<id> '{"status":"active"}'` |
| Suspend | `PATCH /api/users/<id> '{"status":"suspended"}'` |
| Make admin | `PATCH /api/users/<id> '{"role":"admin"}'` |
| Grants | `PUT /api/users/<id>/permissions '{"permissions":["users.read"]}'` |
| Set a password | `PUT /api/users/<id>/password '{"password":"..."}'` |
| Help index, one page | `GET /api/help`, `GET /api/help/<slug>` |

## Errors

The body is `{"error":{"code":"...","message":"...","request_id":"..."}}`. Report the `code` in plain words and the `request_id`. The codes and what they mean are in `docs/system/api.md`. Common ones: `forbidden` (the account lacks the permission), `cannot_change_self`, `last_admin`, `email_taken`, `validation_failed` (see `details.fields`), `too_many_attempts` (wait until `details.retry_at`).

## Safety

- Reading is fine at any time. **Ask before every write** (create, update, suspend, permissions, password) and say exactly what will change for whom.
- A new password for another person is chosen by the person you are helping. Do not invent one and do not echo it back.
- Never print `.env` or the cookie file.
- If the API is not running, say so and suggest `scripts/dev.sh`; do not start servers on your own.
