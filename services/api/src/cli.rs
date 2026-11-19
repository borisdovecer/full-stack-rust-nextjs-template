//! Admin commands run from a terminal: migrations, the first admin, password resets.
//! Passwords are read from the terminal without echo, or from stdin with `--password-stdin`
//! for scripts. Never from an argument, which would land in shell history and `ps`.

use std::io::{BufRead, IsTerminal};

use anyhow::{Context, bail};
use serde_json::json;

use app_auth::password;
use app_db::{MIGRATOR, Pool};

use crate::auth::repository::users::{self, LoginMethod, NewUser};
use crate::auth::repository::{LoginType, Role, UserStatus, sessions};
use crate::shared::repository::audit::{self, action};

/// Runs pending migrations, or reverts the newest one.
pub async fn migrate(db: &Pool, revert_last: bool) -> anyhow::Result<()> {
    if revert_last {
        let applied = app_db::applied_migrations(db)
            .await
            .context("cannot read the applied migrations")?;
        let Some(&last) = applied.first() else {
            bail!("no migration has been applied");
        };
        let target = applied.get(1).copied().unwrap_or(0);
        MIGRATOR.undo(db, target).await.context("revert failed")?;
        println!("reverted migration {last}");
    } else {
        MIGRATOR.run(db).await.context("migration failed")?;
    }
    let state = app_db::migration_state(db).await?;
    println!("{}", state.describe());
    Ok(())
}

pub async fn create_admin(
    db: &Pool,
    email: &str,
    first_name: &str,
    last_name: &str,
    password_stdin: bool,
) -> anyhow::Result<()> {
    let email = crate::shared::validation::normalize_email(email);
    if !email.contains('@') {
        bail!("`{email}` is not an email address");
    }
    let first_name = first_name.trim();
    let last_name = last_name.trim();
    if first_name.is_empty() || last_name.is_empty() {
        bail!("the first and last name must not be empty");
    }
    if users::find_by_email(db, &email).await?.is_some() {
        bail!("a user with the email {email} already exists");
    }

    let pw = read_new_password(password_stdin)?;
    let hash = password::hash(&pw).context("cannot hash the password")?;

    let mut tx = db.begin().await?;
    let user = users::create(
        &mut *tx,
        NewUser {
            email: &email,
            first_name,
            last_name,
            bio: None,
            login_type: LoginType::Password,
            password_hash: Some(&hash),
            wallet: None,
            role: Role::Admin,
            status: UserStatus::Active,
        },
    )
    .await?;
    audit::record(
        &mut *tx,
        None,
        Some(user.id),
        action::USER_CREATED,
        json!({ "role": "admin", "status": "active", "via": "cli" }),
    )
    .await?;
    tx.commit().await?;

    println!("created admin {email} ({})", user.id);
    Ok(())
}

pub async fn reset_password(db: &Pool, email: &str, password_stdin: bool) -> anyhow::Result<()> {
    let email = crate::shared::validation::normalize_email(email);
    let Some(user) = users::find_by_email(db, &email).await? else {
        bail!("no user with the email {email}");
    };

    let pw = read_new_password(password_stdin)?;
    let hash = password::hash(&pw).context("cannot hash the password")?;

    let mut tx = db.begin().await?;
    users::set_login_method(&mut *tx, user.id, LoginMethod::Password { hash: &hash }).await?;
    let revoked = sessions::revoke_all(&mut *tx, user.id, None).await?;
    let mut data = json!({ "via": "cli", "sessions_revoked": revoked });
    if user.login_type != LoginType::Password {
        data["login_type"] = json!({ "before": user.login_type, "after": LoginType::Password });
    }
    audit::record(
        &mut *tx,
        None,
        Some(user.id),
        action::USER_PASSWORD_SET,
        data,
    )
    .await?;
    tx.commit().await?;

    println!("password set for {email}; {revoked} session(s) signed out");
    Ok(())
}

fn read_new_password(from_stdin: bool) -> anyhow::Result<String> {
    let pw = if from_stdin || !std::io::stdin().is_terminal() {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .context("cannot read the password from stdin")?;
        line.trim_end_matches(['\r', '\n']).to_string()
    } else {
        let first = rpassword::prompt_password("New password: ")?;
        let second = rpassword::prompt_password("Repeat it: ")?;
        if first != second {
            bail!("the passwords do not match");
        }
        first
    };
    if let Err(e) = password::check_rules(&pw) {
        bail!("{e}");
    }
    Ok(pw)
}
