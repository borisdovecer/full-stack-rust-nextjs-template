//! The fixed list of permissions. A route asks for a permission, never for a role.
//! Adding one is one line in `permissions!` below, plus deciding which roles get it in
//! [`role_defaults`] and giving it words in the web.

use serde::{Deserialize, Serialize};

/// What a route names to require a permission: `Require<marker::UsersRead>`.
pub trait RequiredPermission: Send + Sync {
    const PERMISSION: Permission;
}

/// Declares every permission once: the enum variant, its wire name, `ALL`, and a marker type
/// in [`marker`] for routes, so no list can miss one. The name is a `tt`, not a `literal`:
/// utoipa does not see a `literal` fragment in `serde(rename)` and would publish the variant.
macro_rules! permissions {
    ($( $(#[$doc:meta])* $variant:ident = $name:tt ),+ $(,)?) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            Serialize,
            Deserialize,
            utoipa::ToSchema,
        )]
        pub enum Permission {
            $( $(#[$doc])* #[serde(rename = $name)] $variant, )+
        }

        impl Permission {
            pub const ALL: &'static [Permission] = &[$(Permission::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $( Permission::$variant => $name, )+
                }
            }
        }

        /// One type per permission, for `Require<P>`.
        pub mod marker {
            $(
                pub struct $variant;
                impl super::RequiredPermission for $variant {
                    const PERMISSION: super::Permission = super::Permission::$variant;
                }
            )+
        }
    };
}

permissions! {
    /// See the user list and user details.
    UsersRead = "users.read",
    /// Create users, change role, status, permissions and passwords.
    UsersManage = "users.manage",
}

impl Permission {
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.as_str() == s)
    }
}

/// What a role has without any grant.
pub fn role_defaults(is_admin: bool) -> &'static [Permission] {
    if is_admin { Permission::ALL } else { &[] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_match_serde() {
        for p in Permission::ALL {
            assert_eq!(Permission::parse(p.as_str()), Some(*p));
            assert_eq!(
                serde_json::to_string(p).ok(),
                Some(format!("\"{}\"", p.as_str()))
            );
        }
        assert_eq!(Permission::parse("users.delete"), None);
    }

    #[test]
    fn the_openapi_schema_publishes_the_wire_names() {
        let schema = serde_json::to_string(&<Permission as utoipa::PartialSchema>::schema())
            .unwrap_or_default();
        for p in Permission::ALL {
            assert!(schema.contains(&format!("\"{}\"", p.as_str())), "{schema}");
        }
        assert!(!schema.contains("UsersRead"), "{schema}");
    }

    #[test]
    fn markers_name_their_permission() {
        assert_eq!(
            <marker::UsersRead as RequiredPermission>::PERMISSION,
            Permission::UsersRead
        );
        assert_eq!(
            <marker::UsersManage as RequiredPermission>::PERMISSION,
            Permission::UsersManage
        );
    }
}
