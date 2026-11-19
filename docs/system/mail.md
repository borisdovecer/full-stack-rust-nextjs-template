# Mail

Last checked against the code on 2026-11-18.

The API sends one kind of email: the 6-digit sign-in codes ([auth](auth.md)). Code: `crates/mail/`, `services/api/src/auth/service/codes.rs`.

## Where mail goes

The API picks one transport when it starts:

| `RESEND_API_KEY` | Transport | `/api/health` says |
|---|---|---|
| set | [Resend](https://resend.com)'s HTTP API, from `mail.from` in `config/app.toml` | `mail: "resend"` |
| empty | the API log, code included, as an `info` line | `mail: "log"` |

The log transport is for development. In production the email-code method is turned off when there is no key, because codes would sit in the server's log files. A server should always report `mail: "resend"`.

`mail.from` is `Name <address>`. Resend sends only from a domain verified in its dashboard, and refuses anything else.

## Sending

Mail leaves in the background, after the API has answered. "Send me a code" therefore takes the same time whether or not a mail goes out, so its timing does not reveal who has an account. A mail Resend refuses is logged as a warning (`sign-in code not sent`, with the code's id and never the code), and that code is retired so the next request sends a new one without waiting.

Messages are plain text. Each says what the code is for (sign in, create an account, change the sign-in method), that it works once for `login_code.lifetime_minutes`, and that it can be ignored.

## Setting it up

1. Create a Resend account and verify the sending domain (DNS records).
2. Create an API key with "sending access" only.
3. Put it in `RESEND_API_KEY` (`.env`, or `/etc/app/api.env` on a server), set `mail.from` to an address on the verified domain, and restart the API.
4. Check `curl -s localhost:8080/api/health` says `"mail":"resend"`, then sign in with a code once.

A new key replaces the old one by editing the variable and restarting; codes already sent keep working.
