# Deploy

Last checked against the code on 2026-11-18.

How the project runs in production and how a change gets there. The step-by-step server guide is `deploy/README.md`.

## On the server

| Piece | Where | Notes |
|---|---|---|
| Caddy | `/etc/caddy/Caddyfile` (from `deploy/Caddyfile`) | ports 80 and 443, TLS from Let's Encrypt, security headers including a Content-Security-Policy (same origin only, plus browser Sentry); `/api/*` to the API, the rest to the web |
| API | `app-api.service`, `/opt/app/current/bin/app-api` | `127.0.0.1:8080`, env `/etc/app/api.env`, settings `/etc/app/app.toml` |
| Web | `app-web.service`, `/opt/app/current/web/server.js` | `127.0.0.1:3000`, Next standalone build |
| PostgreSQL | system package | loopback only, database and role named after the project |
| Backups | `app-backup.timer` → `/opt/app/bin/app-backup` | daily `pg_dump` to `/var/backups/app`, newest 14 kept |
| Firewall | ufw | 22, 80, 443 only |

Both services run as the unprivileged `app` user with systemd hardening (read-only system, no new privileges, private `/tmp`).

## Releases

`.github/workflows/deploy.yml` is started by hand from the Actions tab:

1. Builds the API (`cargo build --release`, with the commit baked into `/api/health`) and the web (`NEXT_OUTPUT=standalone npm run build`).
2. Packs them into `<time>-<commit>.tar.gz` and copies it over ssh as the `deploy` user.
3. Unpacks it into `/opt/app/releases/` and runs `sudo app-activate <release>`, the only root command `deploy` may run. It:
   - runs `app-api migrate` as `app` with the production environment (a failed migration stops here, and the old release keeps running);
   - points `/opt/app/current` at the new release and records it in `/opt/app/history`;
   - restarts both services and waits up to 30 seconds for `/api/health`;
   - deletes old releases, except the last 5 that went live.

Every release that goes live is appended to `/opt/app/history`. `sudo app-activate --rollback` points `current` back at the release that was live before the current one, takes the current one off the history (so a second rollback goes further back), and restarts. A release whose migration failed never went live, so a rollback never lands on it. It does not undo migrations, which is why migrations must work with the code before and after them (`docs/rules/database.md`).

## Configuration in production

`APP_ENV=production` in `/etc/app/api.env`: `Secure` cookies, JSON logs, help docs from inside the binary, and the OpenAPI document only for people with `users.manage`. The API refuses to start while `wallet.domain`, or `oauth.public_url` with a provider on, still points at this machine (`127.0.0.1`, `localhost`), and when that URL is not `https://`.

Sizing: each API process opens up to `DATABASE_MAX_CONNECTIONS` (10) connections. With several processes, keep their sum below Postgres' `max_connections`, or put PgBouncer in front. A second API instance needs nothing else: sessions and rate limits live in Postgres, and the hourly clean-up runs on one instance at a time (an advisory lock). `NEXT_PUBLIC_SENTRY_DSN` for the browser is a build-time value: set it as a repository variable for the deploy workflow.

## Checking a server

```bash
systemctl status app-api app-web caddy
curl -s http://127.0.0.1:8080/api/health
journalctl -u app-api --since "1 hour ago"
ls -1t /var/backups/app | head
```
