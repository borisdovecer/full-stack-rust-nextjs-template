//! What the help routes send.

use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct HelpEntry {
    pub slug: String,
    pub title: String,
}
