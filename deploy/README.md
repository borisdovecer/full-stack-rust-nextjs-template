# Deploy

One Ubuntu server (24.04 or newer) runs everything: Caddy for TLS, the API and the web as systemd services, PostgreSQL on loopback, a daily backup. Releases are built on GitHub and copied over ssh. No containers.

```
/opt/app/
  releases/<time>-<commit>/   one folder per release: bin/app-api, web/ (Next standalone)
  current -> releases/...     the live release
  incoming/                   uploads land here
  bin/app-backup              the backup script
/etc/app/
  api.env, web.env            environment (mode 640, root:app)
  app.toml                    settings (copy of config/app.toml, edited for production)
/var/backups/app/             daily database dumps, newest 14
```

Names below are the template's placeholders; `scripts/rename.sh` replaces them with your project's name.

## 1. Server setup (once)

As root on a fresh server. Replace `example.com` with your host name, and point its DNS A/AAAA records at the server first.

```bash
# Packages
apt update && apt upgrade -y
apt install -y postgresql caddy curl ufw
curl -fsSL https://deb.nodesource.com/setup_26.x | bash - && apt install -y nodejs

# Firewall: ssh, http, https only
ufw default deny incoming && ufw allow 22/tcp && ufw allow 80/tcp && ufw allow 443/tcp && ufw enable

# Users: `app` runs the services, `deploy` uploads releases
useradd --system --home /opt/app --shell /usr/sbin/nologin app
useradd --create-home --shell /bin/bash deploy
install -d -m 700 -o deploy -g deploy /home/deploy/.ssh
# Put the deploy public key into /home/deploy/.ssh/authorized_keys (mode 600, owner deploy)

# Folders. /opt/app holds no secrets; releases are written by `deploy` and read by `app`.
install -d -o root -g root -m 755 /opt/app /opt/app/bin
install -d -o deploy -g app -m 750 /opt/app/releases
install -d -o deploy -g deploy -m 700 /opt/app/incoming
install -d -o root -g app -m 750 /etc/app
install -d -o app -g app -m 700 /var/backups/app
```

Copy files from this repository to the server (for example with `scp`), then as root:

```bash
install -m 755 deploy/app-activate /usr/local/bin/app-activate
install -m 755 deploy/app-backup /opt/app/bin/app-backup
install -m 440 deploy/sudoers-app-deploy /etc/sudoers.d/app-deploy && visudo -c
install -m 644 deploy/app-api.service deploy/app-web.service deploy/app-backup.service deploy/app-backup.timer /etc/systemd/system/
install -m 644 deploy/Caddyfile /etc/caddy/Caddyfile
echo 'APP_DOMAIN=example.com' > /etc/caddy/caddy.env
mkdir -p /etc/systemd/system/caddy.service.d
printf '[Service]\nEnvironmentFile=/etc/caddy/caddy.env\n' > /etc/systemd/system/caddy.service.d/env.conf
install -d -o caddy -g caddy /var/log/caddy
```

Database and settings:

```bash
pw=$(openssl rand -base64 32 | tr -d '/+=')
sudo -u postgres psql -c "CREATE ROLE app LOGIN PASSWORD '$pw'"
sudo -u postgres createdb -O app app

install -m 640 -o root -g app deploy/api.env.example /etc/app/api.env
sed -i "s|CHANGE_ME|$pw|" /etc/app/api.env
install -m 640 -o root -g app deploy/web.env.example /etc/app/web.env
install -m 640 -o root -g app config/app.toml /etc/app/app.toml   # edit app.name, registration.mode, mail.from, wallet.domain, oauth.public_url
unset pw

systemctl daemon-reload
systemctl enable app-api app-web app-backup.timer
systemctl restart caddy
```

PostgreSQL listens on loopback only by default; keep it that way.

## 2. First deploy

1. In GitHub, add the secrets listed at the top of `.github/workflows/deploy.yml` (`ssh-keyscan example.com` gives `DEPLOY_KNOWN_HOSTS`).
2. Actions → Deploy → Run workflow. It builds, uploads, and runs `app-activate`, which migrates, switches `/opt/app/current`, restarts both services and waits for `/api/health`.
3. Create the first admin on the server:

```bash
sudo -u app bash -c 'set -a; . /etc/app/api.env; set +a; /opt/app/current/bin/app-api create-admin --email you@example.com --first-name Your --last-name Name'
```

4. Open `https://example.com`.

## 3. Everyday

| Task | How |
|---|---|
| Deploy | Actions → Deploy → Run workflow |
| Roll back the code | `sudo app-activate --rollback` (switches to the release that was live before, from `/opt/app/history`; run it again to go further back; migrations are not undone, so they must be backwards compatible: `docs/rules/database.md`) |
| Logs | `journalctl -u app-api -f`, `journalctl -u app-web -f`, `/var/log/caddy/app.log` |
| Status | `systemctl status app-api app-web`, `curl -s localhost:8080/api/health` (`"mail":"resend"` when sign-in codes go out) |
| Turn on Google or GitHub | create the OAuth app with the callback `https://<host>/api/auth/oauth/<provider>/callback`, set `oauth.public_url` and `oauth.<provider>.client_id` in `/etc/app/app.toml` and `<PROVIDER>_CLIENT_SECRET` in `/etc/app/api.env`, `systemctl restart app-api`; see `docs/system/auth.md` |
| Turn on email codes | put the Resend key in `RESEND_API_KEY` in `/etc/app/api.env`, set `mail.from` in `/etc/app/app.toml`, `systemctl restart app-api`; see `docs/system/mail.md` |
| Reset a password | `sudo -u app bash -c 'set -a; . /etc/app/api.env; set +a; /opt/app/current/bin/app-api reset-password --email someone@example.com'` |
| Change a setting | edit `/etc/app/app.toml`, then `systemctl restart app-api` |
| Run a backup now | `systemctl start app-backup` |

## 4. Restore a backup

```bash
systemctl stop app-web app-api
sudo -u postgres dropdb app && sudo -u postgres createdb -O app app
sudo -u postgres pg_restore --no-owner --role=app -d app /var/backups/app/app-<stamp>.dump
systemctl start app-api app-web
```

Copy backups off the server too (another machine or object storage): a backup on the same disk does not survive the disk.
