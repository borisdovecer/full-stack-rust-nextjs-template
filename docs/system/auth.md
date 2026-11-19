# Authentication and permissions

Last checked against the code on 2026-11-18.

Accounts, sign-in methods, sessions, roles and permissions. Code: `services/api/src/auth/` (`session.rs` for sessions, `service/login_methods.rs`, `handlers.rs`), `services/api/src/me/`, `services/api/src/users/`, `crates/auth/`.

## Accounts

| Field | Values |
|---|---|
| `first_name`, `last_name` | required, 1 to 100 characters after trimming |
| `bio` | optional, up to 1000 characters; blank clears it |
| `login_type` | how the account signs in: `password`, `wallet`, `code`, `external` (see below) |
| `role` | `user`, `admin` |
| `status` | `pending` (waiting for an admin), `active`, `suspended` (cannot sign in) |

Emails are stored lowercased and trimmed; `Ana@Example.com` and `ana@example.com` are one account.

**Registration mode** (`[registration] mode` in `config/app.toml`):

| Mode | `POST /api/auth/register` |
|---|---|
| `open` | creates an `active` user and signs them in |
| `approval` | creates a `pending` user and signs them in; they see only the waiting page until an admin makes them `active` |
| `closed` | answers `403 registration_closed`; admins create users |

The first admin comes from the CLI: `app-api create-admin --email ... --first-name ... --last-name ...` (`scripts/setup.sh` runs it with the name `admin dev`, asking only for the email and password, and writes that admin's email and password to `.env` as `ADMIN_EMAIL` and `ADMIN_PASSWORD` for `skills/app-api`).

## Sign-in methods

Every account has exactly one method, `users.login_type`, and every method starts with the email.

| Method | Sign in with | Available |
|---|---|---|
| `password` | email and password | yes |
| `code` | email, then a 6-digit code sent to it | yes, when mail is set up (always in development; see [mail](mail.md)) |
| `wallet` | email, then a Solana wallet signs a one-time message | yes |
| `external` | Google or GitHub | yes, for each provider that is configured |

`GET /api/config` lists the available methods in `login_methods`, and the web draws only those. A method is available once its routes exist and its settings are complete (`auth/service/login_methods.rs`). Register, login and a method change refuse any other method with `422 login_method_unavailable`.

`POST /api/auth/register` and `POST /api/auth/login` take `method` (default `password`) and that method's proof. Login answers `401 invalid_credentials` the same way, and in the same time, for an unknown email, an account that uses another method, and a wrong proof, so the answer never says how an email signs in.

**Changing the method.** A signed-in person changes theirs with `PUT /api/me/login-method`. The session must have signed in within `session.reauth_minutes` (10), or the answer is `403 reauth_required` and the person signs in again: someone at an unlocked, signed-in browser cannot take the account over. The new method is proven in the same request (a new password, a code, a wallet signature). Choosing the method they already have answers `409 login_method_unchanged`; a password is changed at `/api/me/password`. On success the old method's credential is cleared (`password_hash` or `wallet` set to null), every other session ends, and `user.login_method_changed` is written to the audit log.

**Recovery by an admin.** Someone who lost their wallet or their provider account cannot prove anything, so a person with `users.manage` changes it for them: `PUT /api/users/{id}/login-method` with `password` (and a new password) or `code`. An admin gives only those two, listed in `admin_login_methods` in `GET /api/config`, because they need no secret from the person. It ends all of that person's sessions. Nobody changes their own method there (`409 cannot_change_self`), and only an admin changes an admin's (`403 forbidden`). Creating a user (`POST /api/users`) takes the same two methods.

Setting a password always makes `password` the method: `PUT /api/users/{id}/password` and `app-api reset-password` switch an account that used another method, and the audit event records the change.

## Email codes

The `code` method mails a 6-digit code and signs in with it. Code: `services/api/src/auth/service/codes.rs`, `crates/auth/src/code.rs`, `services/api/src/auth/repository/login_codes.rs`.

1. `POST /api/auth/code {email, purpose}` asks for a code. `purpose` is `login` (default) or `register`. The answer is always `204`, for an address with an account, with another method, or with none, and the mail leaves in the background so the answer also takes the same time. A code is mailed only to a `code` account that is not suspended (`login`), or to anyone (`register`, refused with `403 registration_closed` when registration is closed).
2. `POST /api/auth/login {method: "code", email, login_code}` signs in. `POST /api/auth/register {method: "code", ..., login_code}` creates a `code` account; a taken email is reported only after a right code, when the person has proven they hold the inbox.
3. A signed-in person switches to codes with `POST /api/me/login-code` (mails their own address, purpose `change`) and then `PUT /api/me/login-method {method: "code", login_code}`. Both need a recent sign-in.

Rules, with defaults from `[login_code]` in `config/app.toml`:

- A code is six digits from the operating system's random source, every value equally likely. Only a SHA-256 salted with the row id is stored. A code works once, for 10 minutes, and only for the purpose it was sent for. A new code for the same address and purpose retires the older one.
- Three wrong tries end the code and pause the address for 15 minutes (`429 too_many_attempts`). These failures are counted under the `code` method, so they never use up the password's tries. A guess when no code is live counts nothing, so nobody can pause an address by posting codes at it.
- At most one mail per address per 60 seconds: a second request in that time answers `204` and sends nothing. One IP may ask for 10 codes per 10 minutes across addresses (`429`), so the route cannot be used to flood inboxes.
- Every failure answers `401 invalid_code`, the same for a wrong, expired, spent or never-sent code.
- The code is never logged (except by the development log mailer), never in an error body, and the request field is named `login_code`.

## Solana wallets

The `wallet` method signs in with the email and a signature from the Solana wallet stored in `users.wallet`. The signature is the proof the person owns the wallet; no email code is sent. Code: `services/api/src/auth/service/wallet.rs`, `crates/auth/src/siws.rs`, `services/api/src/auth/repository/wallet_nonces.rs`, `web/src/components/WalletConnect/`.

1. The browser connects a wallet (Phantom, Solflare or any wallet-standard wallet the browser has) and asks `POST /api/auth/wallet/nonce {address}` for a challenge: domain, address, statement, a one-time nonce, issued-at and expiry (`wallet.nonce_minutes`, 5).
2. The wallet signs a Sign-In-With-Solana message built from it: through the wallet's own `signIn` when it has one, otherwise `signMessage` over the same text (`createSignInMessageText`). Signing is free; nothing touches the chain.
3. The browser sends the address, the signed bytes (`message`) and the `signature`, both base64, with `POST /api/auth/login {method: "wallet", email, ...}`, `POST /api/auth/register {method: "wallet", ...}`, or `PUT /api/me/login-method {method: "wallet", ...}`.

The API then:

- reads the nonce from the message and spends it first (it must have been issued for that address, unused and unexpired), so a refused signature cannot be tried again;
- checks the message names `wallet.domain` (from `config/app.toml`, not the request, which arrives on loopback behind the proxy; the web also compares it with its own address before asking the wallet, which would refuse to sign otherwise, and names the right address), the same address and that nonce, that its Issued At is within 10 minutes of the server clock and its Expiration Time has not passed;
- verifies the 64-byte Ed25519 signature over the exact bytes against the address's key;
- for login, checks the account with that email uses `wallet` with that address.

Login answers `401 invalid_credentials` for every failure, the same as an unknown email or another method. Registering and switching answer `401 invalid_signature`, and `409 wallet_taken` when another account has the address (`users_wallet_key`). The reason a signature was refused is logged (`wallet proof refused`), never the signature. One IP may ask for `wallet.max_nonces_per_ip` (30) challenges per `nonce_minutes`.

A person who lost their wallet is moved to a password or email codes by an admin (recovery, above); the old address is cleared and free for another account.

## Google and GitHub

The `external` method signs in through Google or GitHub. Code: `services/api/src/auth/oauth/` (`providers.rs` for the provider calls, `handlers.rs` and `cookies.rs` for the round trip), `services/api/src/auth/repository/identities.rs`, `services/api/src/auth/oauth/repository.rs`, `web/src/components/ProviderButtons/`.

**Turning a provider on.** Each is on when its `client_id` (`[oauth.google]`, `[oauth.github]` in `config/app.toml`) and its secret (`GOOGLE_CLIENT_SECRET`, `GITHUB_CLIENT_SECRET`) are both set. Off, its routes answer `404` and the web draws no button; `GET /api/config` lists the providers that are on in `oauth_providers`.

1. In Google Cloud: APIs & Services → Credentials → OAuth client ID, type "Web application". Authorised redirect URI: `{oauth.public_url}/api/auth/oauth/google/callback`.
2. In GitHub: Settings → Developer settings → OAuth Apps → New. Authorization callback URL: `{oauth.public_url}/api/auth/oauth/github/callback`.
3. `oauth.public_url` is the address people open the app on (`https://app.example.com`; `http://127.0.0.1:3000` in development, where Next forwards `/api` to the API). It must be the address in the browser's address bar, or the flow cookie is set on one host and the provider sends the browser back to another.

**The round trip.** The whole flow runs in the API, because the web has no route handlers.

1. `GET /api/auth/oauth/{provider}/start?intent=login|register|link` sends the browser to the provider with a random `state` and a PKCE challenge (S256). It keeps the provider, the intent, the state and the PKCE verifier in an httpOnly, `SameSite=Lax` cookie (`<session cookie>_oauth`, path `/api/auth/oauth`, 10 minutes).
2. The provider sends the browser back to `GET /api/auth/oauth/{provider}/callback?code&state`. The callback refuses a missing cookie, another provider, or a `state` that does not match, so it only works in the browser that started it. It exchanges the code with the verifier and reads the account: Google's userinfo (`sub`, `email`, `email_verified`, names), or GitHub's `/user` (`id`, `name`) and the primary email from `/user/emails` (GitHub has no OpenID `email_verified`). An email the provider has not verified is refused (`email_not_verified`). A cancel at the provider returns to the page with nothing to say.
3. Every outcome is a redirect to a web page, with `?error=<code>` when it failed.

**What each intent does.**

- `login`: the account tied to that provider account (`user_identities`, by the provider's own user id, so a changed email at the provider keeps working). An `external` account with nothing tied yet is tied by its verified email. Anything else, including an account that uses another method, goes to `/login?error=invalid_credentials`. Success opens a session and goes to `/`.
- `register`: refused when registration is closed or the email or provider account already has an account (`email_taken`, said only after the provider vouched for the email). Otherwise the provider's answer waits in `oauth_signups` for 30 minutes behind a random token in an httpOnly cookie (`<session cookie>_signup`, path `/api/auth`; only its SHA-256 is stored), and the browser goes to `/register?via=external`. The form reads it with `GET /api/auth/oauth/signup` (email, names), asks for the names the provider did not give, and sends `POST /api/auth/register {method: "external", first_name, last_name, bio}`. The email and the provider account come from the cookie, never the body; the answer is used once.
- `link`: Settings → "Use Google or GitHub instead". Needs a signed-in, active session that signed in recently (checked at `start` and again at the callback), and the provider's verified email must be the account's (`provider_email_mismatch` otherwise, also when that provider account belongs to another account). The account becomes `external`, its old credential is cleared, the provider account is tied, other sessions end, and `user.login_method_changed` records the provider.

One account has one provider account. Changing the method in any way (the person, an admin's recovery) unties it, so it can be tied to another account later. Admins do not give `external` accounts; a person sets it up themselves.

## Passwords

- Argon2id with the library's defaults (OWASP's recommended parameters), a fresh salt per hash.
- Rule: 12 to 128 characters, counted as characters. No other composition rules.
- Hashing runs on a blocking thread, off the async workers, at most `password.max_concurrent_hashes` at once (default: twice the CPU cores). Each hash holds about 19 MiB, so a burst of logins waits its turn instead of using up the server's memory. Code: `services/api/src/auth/service/passwords.rs`.
- There is no self-service reset. An admin sets a new password (`PUT /api/users/{id}/password`), or the CLI does (`app-api reset-password --email ...`). Both sign the user out everywhere and make `password` the account's method. Nobody sets their own password through the admin route (`409 cannot_change_self`); their own is changed at `POST /api/me/password`, with the current one. A wrong current password counts as a failed password login (throttling, below), so a stolen session cannot guess it freely.

## Sessions

- Login creates a row in `user_sessions` and sets a cookie holding a random 32-byte token (base64url). The database keeps only its SHA-256, so a copy of the database signs nobody in.
- Cookie: name from `session.cookie_name`, `HttpOnly`, `SameSite=Lax`, `Path=/`, `Secure` in production, `Max-Age` = `session.lifetime_hours` (7 days).
- A session used after `session.extend_after_hours` (24 h) since it was last extended gets a fresh lifetime, and the cookie is set again. An active user stays signed in.
- `last_seen_at` is written at most every 5 minutes.
- A session ends when it expires, on logout, on "log out everywhere", when its user changes their password or sign-in method (other sessions only), when an admin sets their password or method, or when they are suspended. Each check is a database lookup, so it takes effect at once.
- A cookie that no longer matches a valid session is cleared in the same response.
- When the session cannot be looked up (database down or busy), the request goes on without a user and the cookie is kept: public routes (`/api/health`, `/api/config`) answer as usual, and routes that need a session answer `503 unavailable`, not `401`, so an outage signs nobody out.
- Ended sessions are deleted a day later by the hourly clean-up.

## Login throttling

Before checking a proof, the API counts recent failures:

| Limit | Setting | Default |
|---|---|---|
| failures for one email with one method | `login_throttle.max_failures` | 5 |
| failures from one IP, across emails | `login_throttle.max_failures_per_ip` | 20 |
| window | `login_throttle.window_minutes` | 15 |

`POST /api/me/password` checks the current password under the same limits, counted as `password` failures for the account's email. Over a limit: `429 too_many_attempts` with `details.retry_at`. Each attempt records its method (`auth_login_attempts.method`), so failures with one method never use up another method's tries. A successful login clears the count for that email and method. Attempts older than 30 days are deleted.

The limits hold for requests sent at the same moment. An attempt is written as a failure *before* its proof is checked, then counted together with every other attempt in flight, and marked a success only when the proof passes; an attempt refused for the limit is removed again. A mailed code's tries are taken in the same statement that checks one is left. Code: `services/api/src/auth/service/throttle.rs`, `login_codes::take_try`.

"No such email" and "wrong password" answer the same (`401 invalid_credentials`) and take the same time: an unknown email is checked against a dummy hash.

The client IP is the peer address, or the last `X-Forwarded-For` entry when the peer is on loopback (Caddy, or Next in development). Do not expose the API directly on a public address with a proxy in front that you do not control.

## Permissions

A route asks for a permission, never for a role. The list is fixed in code (`crates/auth/src/permission.rs`):

| Permission | Allows |
|---|---|
| `users.read` | the user list and user details |
| `users.manage` | create users, change role, status, permissions, set passwords |

An admin has every permission. A user has none, plus whatever is granted to them in `user_permissions`. Effective permissions are returned by `GET /api/me` so the web can hide what a person cannot use; the API checks every request anyway.

Rules for changing users, enforced by the API:

- Nobody changes their own role, status, permissions, sign-in method or password through the user routes (`409 cannot_change_self`).
- Only an admin changes an admin's account or makes someone an admin (`403 forbidden`). This keeps `users.manage` from being a way to become admin.
- The last active admin cannot be demoted or suspended (`409 last_admin`).
- Every change writes an `audit_events` row in the same transaction.

To add a permission: add one line to `permissions!` in `crates/auth/src/permission.rs` (`/// what it allows` then `WalletsRead = "wallets.read",`). That declares the enum variant, its name, its place in `Permission::ALL`, and the marker type routes use: `Require<WalletsRead>`. Then decide in `role_defaults` which roles get it, add its words in `web/src/lib/labels.ts`, and regenerate the web's API types. No migration: grants are stored by name.

The user detail (`GET /api/users/{id}`) lists each grant with who granted it and when (`user_permissions.granted_by`, `granted_at`).

## Extractors

| Extractor | Lets through | Otherwise |
|---|---|---|
| `SessionUser` | any signed-in user, `pending` included | `401 unauthenticated`, or `503 unavailable` when the session could not be looked up |
| `CurrentUser` | an `active` signed-in user | `401`, or `403 account_pending` |
| `Require<UsersRead>` | an active user with the permission | `401`, `403 account_pending`, `403 forbidden` |

## Cross-site request forgery

Two layers: the cookie is `SameSite=Lax`, and every write must be `Content-Type: application/json`, which a cross-site form cannot send. The web client sends that header on every request.
