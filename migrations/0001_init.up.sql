-- 0001_init: the whole schema the template starts with: accounts, preferences, permissions,
-- sessions, sign-in (passwords, email codes, Solana wallets, Google and GitHub), login
-- throttling and the audit log. Every table is owned by the API service (services/api).
--
-- The tables are empty when this runs, so indexes are built here without CONCURRENTLY. An index
-- added later to a table with rows goes in a migration of its own (docs/rules/database.md).

-- Trigram matching, so the admin user search ("anywhere in the email or name") uses an index.
-- A trusted extension since PG 13: the database owner may create it without superuser rights.
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TYPE user_role AS ENUM ('user', 'admin');
CREATE TYPE user_status AS ENUM ('pending', 'active', 'suspended');
CREATE TYPE theme AS ENUM ('system', 'light', 'dark');
CREATE TYPE login_type AS ENUM ('password', 'wallet', 'code', 'external');
CREATE TYPE login_code_purpose AS ENUM ('login', 'register', 'change');

-- Keeps updated_at current on every UPDATE. Reused by every table that has the column.
CREATE FUNCTION set_updated_at() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END
$$;

-- Accounts.
--
-- Emails are stored lowercased and trimmed (the API normalises them, the CHECK enforces it), so
-- the plain UNIQUE constraint is case-insensitive in practice. The API maps a violation of
-- `users_email_key` to the `email_taken` error.
CREATE TABLE users (
    id            uuid         PRIMARY KEY DEFAULT gen_random_uuid(),
    email         varchar(255) NOT NULL,
    first_name    varchar(255) NOT NULL,
    last_name     varchar(255) NOT NULL,
    bio           text,
    login_type    login_type   NOT NULL DEFAULT 'password',
    wallet        varchar(64),
    password_hash varchar(255),
    role          user_role    NOT NULL DEFAULT 'user',
    status        user_status  NOT NULL DEFAULT 'active',
    last_login_at timestamptz,
    created_at    timestamptz  NOT NULL DEFAULT now(),
    updated_at    timestamptz  NOT NULL DEFAULT now(),
    CONSTRAINT users_email_key UNIQUE (email),
    CONSTRAINT users_email_normalised CHECK (email = lower(btrim(email)) AND email <> ''),
    CONSTRAINT users_first_name_not_blank CHECK (btrim(first_name) <> ''),
    CONSTRAINT users_last_name_not_blank CHECK (btrim(last_name) <> ''),
    CONSTRAINT users_bio_length CHECK (char_length(bio) <= 1000),
    -- Each sign-in method needs its credential; the other methods' columns may stay null.
    CONSTRAINT users_login_type_credentials CHECK (
        (login_type <> 'password' OR password_hash IS NOT NULL)
        AND (login_type <> 'wallet' OR wallet IS NOT NULL)
    )
);

CREATE UNIQUE INDEX users_wallet_key ON users (wallet) WHERE wallet IS NOT NULL;
CREATE INDEX users_created_at_idx ON users (created_at DESC);
CREATE INDEX users_role_status_idx ON users (role, status);
-- The user list: search inside emails and inside "first last" (the expression it searches),
-- and its name and last-login sorts.
CREATE INDEX users_email_trgm_idx ON users USING gin (email gin_trgm_ops);
CREATE INDEX users_full_name_trgm_idx
    ON users USING gin ((first_name || ' ' || last_name) gin_trgm_ops);
CREATE INDEX users_name_sort_idx ON users (lower(first_name), lower(last_name), id);
CREATE INDEX users_last_login_at_idx ON users (last_login_at DESC NULLS LAST, id);

CREATE TRIGGER users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- One row per user, created in the same statement as the user. A new preference is a new
-- column with a default, so existing rows stay valid.
CREATE TABLE user_preferences (
    user_id    uuid        PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    theme      theme       NOT NULL DEFAULT 'system',
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TRIGGER user_preferences_set_updated_at
    BEFORE UPDATE ON user_preferences
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Grants on top of a user's role. `permission` holds a name from the API's fixed list
-- (crates/auth/src/permission.rs). The database does not know the list on purpose: adding a
-- permission must not need a migration.
CREATE TABLE user_permissions (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    permission varchar(64) NOT NULL,
    granted_by uuid        REFERENCES users (id) ON DELETE SET NULL,
    granted_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, permission)
);

-- One row per signed-in browser. The cookie holds a random token; only its SHA-256 is stored
-- here. A session is valid while `revoked_at` is null and `expires_at` is in the future.
CREATE TABLE user_sessions (
    id           uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash   bytea       NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz NOT NULL,
    revoked_at   timestamptz,
    ip           text,
    user_agent   text,
    CONSTRAINT user_sessions_token_hash_key UNIQUE (token_hash)
);

CREATE INDEX user_sessions_user_id_idx ON user_sessions (user_id);
CREATE INDEX user_sessions_expires_at_idx ON user_sessions (expires_at);
-- For the hourly clean-up; partial, as most sessions are never revoked.
CREATE INDEX user_sessions_revoked_at_idx
    ON user_sessions (revoked_at) WHERE revoked_at IS NOT NULL;

-- Every login attempt, for throttling by email and by IP. Failures are counted per email and
-- method, so wrong codes never use up the password's tries; the IP limit counts across
-- methods. Rows older than 30 days are deleted by the API's housekeeping task.
CREATE TABLE auth_login_attempts (
    id           bigserial    PRIMARY KEY,
    email        varchar(255) NOT NULL,
    ip           text,
    success      boolean      NOT NULL,
    attempted_at timestamptz  NOT NULL DEFAULT now(),
    method       login_type   NOT NULL DEFAULT 'password'
);

CREATE INDEX auth_login_attempts_email_idx
    ON auth_login_attempts (email, method, attempted_at DESC);
CREATE INDEX auth_login_attempts_ip_idx ON auth_login_attempts (ip, attempted_at DESC);
CREATE INDEX auth_login_attempts_attempted_at_idx ON auth_login_attempts (attempted_at);

-- Who changed what about whom. Written in the same transaction as the change it records.
-- `data` holds before and after values and never a password, hash or token. A null actor is
-- the system or the CLI.
CREATE TABLE audit_events (
    id         bigserial   PRIMARY KEY,
    actor_id   uuid        REFERENCES users (id) ON DELETE SET NULL,
    subject_id uuid        REFERENCES users (id) ON DELETE SET NULL,
    action     varchar(64) NOT NULL,
    data       jsonb       NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX audit_events_subject_idx ON audit_events (subject_id, created_at DESC);
CREATE INDEX audit_events_created_at_idx ON audit_events (created_at DESC);

-- The 6-digit codes the `code` sign-in method mails. One row per code asked for, whether or
-- not an account uses the address, so the answer to "send me a code" is the same either way.
-- Only a salted SHA-256 of the code is kept; the row id is the salt and is chosen by the API.
-- A new code for the same address and purpose marks the older ones used. Rows are deleted a
-- day after they expire.
CREATE TABLE login_codes (
    id         uuid               PRIMARY KEY,
    email      varchar(255)       NOT NULL,
    purpose    login_code_purpose NOT NULL,
    code_hash  bytea              NOT NULL,
    attempts   integer            NOT NULL DEFAULT 0,
    ip         text,
    expires_at timestamptz        NOT NULL,
    used_at    timestamptz,
    created_at timestamptz        NOT NULL DEFAULT now()
);

CREATE INDEX login_codes_email_idx ON login_codes (email, purpose, created_at DESC);
CREATE INDEX login_codes_ip_idx ON login_codes (ip, created_at DESC);
CREATE INDEX login_codes_expires_at_idx ON login_codes (expires_at);

-- One-time challenges a Solana wallet signs to sign in. Issued for an address, valid a few
-- minutes, spent before the signature is checked, so a refused signature cannot be retried
-- with the same nonce. Deleted a day after they expire.
CREATE TABLE wallet_nonces (
    nonce      varchar(64)  PRIMARY KEY,
    address    varchar(64)  NOT NULL,
    ip         text,
    issued_at  timestamptz  NOT NULL DEFAULT now(),
    expires_at timestamptz  NOT NULL,
    used_at    timestamptz
);

CREATE INDEX wallet_nonces_expires_at_idx ON wallet_nonces (expires_at);
CREATE INDEX wallet_nonces_ip_idx ON wallet_nonces (ip, issued_at DESC);

-- Ties an account to one Google or GitHub account by the provider's own user id (`subject`),
-- so changing the email at the provider does not lose the account. One identity per account,
-- as one account has one sign-in method; switching away deletes it.
CREATE TABLE user_identities (
    user_id    uuid         NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider   varchar(32)  NOT NULL,
    subject    varchar(255) NOT NULL,
    email      varchar(255) NOT NULL,
    created_at timestamptz  NOT NULL DEFAULT now(),
    PRIMARY KEY (provider, subject),
    CONSTRAINT user_identities_user_id_key UNIQUE (user_id)
);

-- A provider's verified answer, held between the callback and the registration form for 30
-- minutes. The browser holds a random token; only its SHA-256 is stored.
CREATE TABLE oauth_signups (
    token_hash bytea        PRIMARY KEY,
    provider   varchar(32)  NOT NULL,
    subject    varchar(255) NOT NULL,
    email      varchar(255) NOT NULL,
    first_name varchar(255),
    last_name  varchar(255),
    expires_at timestamptz  NOT NULL,
    created_at timestamptz  NOT NULL DEFAULT now()
);

CREATE INDEX oauth_signups_expires_at_idx ON oauth_signups (expires_at);
