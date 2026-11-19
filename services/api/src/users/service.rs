//! User management logic the handlers share: the detail view, the audit diff, the admin check.

use serde_json::{Value, json};

use app_auth::Permission;
use app_core::AppResult;

use super::dto::{AuditEventDto, GrantDto, UserDetail};
use crate::auth::repository::{Role, User, permissions};
use crate::auth::{Authenticated, combine_permissions};
use crate::shared::repository::audit;
use crate::state::AppState;

const AUDIT_EVENTS_SHOWN: i64 = 20;

pub(super) async fn load_detail(state: &AppState, user: User) -> AppResult<UserDetail> {
    let rows = permissions::list_grants(&state.db, user.id).await?;
    let names: Vec<&str> = rows.iter().map(|g| g.permission.as_str()).collect();
    let permissions = combine_permissions(&user, &names);
    let grants = rows
        .iter()
        .filter_map(|g| {
            Some(GrantDto {
                permission: Permission::parse(&g.permission)?,
                granted_by: g.granted_by.clone(),
                granted_at: g.granted_at,
            })
        })
        .collect();
    let audit = audit::list_for_subject(&state.db, user.id, AUDIT_EVENTS_SHOWN)
        .await?
        .into_iter()
        .map(AuditEventDto::from)
        .collect();
    Ok(UserDetail {
        user: user.into(),
        permissions,
        grants,
        audit,
    })
}

/// `{ field: { before, after } }` for each field that changed.
pub(super) fn diff(before: &User, after: &User) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    if before.first_name != after.first_name {
        out.insert(
            "first_name".into(),
            json!({ "before": before.first_name, "after": after.first_name }),
        );
    }
    if before.last_name != after.last_name {
        out.insert(
            "last_name".into(),
            json!({ "before": before.last_name, "after": after.last_name }),
        );
    }
    if before.bio != after.bio {
        out.insert(
            "bio".into(),
            json!({ "before": before.bio, "after": after.bio }),
        );
    }
    if before.role != after.role {
        out.insert(
            "role".into(),
            json!({ "before": before.role, "after": after.role }),
        );
    }
    if before.status != after.status {
        out.insert(
            "status".into(),
            json!({ "before": before.status, "after": after.status }),
        );
    }
    out
}

pub(super) fn is_admin(a: &Authenticated) -> bool {
    a.user.role == Role::Admin
}
