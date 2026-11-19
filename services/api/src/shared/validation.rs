//! Input checks shared by scopes. Problems are collected per field and returned together as
//! `validation_failed` with `details.fields`, so a form can mark every bad field at once.

use std::collections::BTreeMap;

use serde_json::json;

use app_core::{AppError, ErrorCode};

pub const NAME_MAX: usize = 100;
pub const BIO_MAX: usize = 1000;
const EMAIL_MAX: usize = 255;

/// Lowercased and trimmed: the only form an email is stored or looked up in.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

#[derive(Debug, Default)]
pub struct FieldErrors(BTreeMap<&'static str, Vec<&'static str>>);

impl FieldErrors {
    pub fn add(&mut self, field: &'static str, problem: &'static str) {
        self.0.entry(field).or_default().push(problem);
    }

    pub fn check(self) -> Result<(), AppError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(AppError::new(ErrorCode::ValidationFailed)
                .with_details(json!({ "fields": self.0 })))
        }
    }

    /// Normalises the email, or records why it cannot be one.
    pub fn email(&mut self, field: &'static str, raw: &str) -> String {
        let email = normalize_email(raw);
        if email.is_empty() {
            self.add(field, "required");
        } else if email.chars().count() > EMAIL_MAX {
            self.add(field, "too_long");
        } else if !looks_like_email(&email) {
            self.add(field, "invalid_email");
        }
        email
    }

    /// Trims the name, or records why it cannot be one.
    pub fn name(&mut self, field: &'static str, raw: &str) -> String {
        let name = raw.trim().to_string();
        if name.is_empty() {
            self.add(field, "required");
        } else if name.chars().count() > NAME_MAX {
            self.add(field, "too_long");
        }
        name
    }

    /// Trims the bio; an empty one is `None`. Records why it cannot be one.
    pub fn bio(&mut self, field: &'static str, raw: &str) -> Option<String> {
        let bio = raw.trim();
        if bio.chars().count() > BIO_MAX {
            self.add(field, "too_long");
        }
        (!bio.is_empty()).then(|| bio.to_string())
    }
}

/// One `@`, something before it, a dot inside the domain, no spaces. Deliverability is not
/// checked here; a mailed code proves the inbox where it matters.
fn looks_like_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !s.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_normalising() {
        assert_eq!(normalize_email("  Ana@Example.COM "), "ana@example.com");
    }

    #[test]
    fn emails() {
        for ok in ["a@b.co", "first.last+tag@sub.example.org"] {
            assert!(looks_like_email(ok), "{ok}");
        }
        for bad in [
            "", "a", "a@b", "@b.co", "a@@b.co", "a@.co", "a@b.", "a b@c.co",
        ] {
            assert!(!looks_like_email(bad), "{bad}");
        }
    }

    #[test]
    fn errors_collect_per_field() {
        let mut f = FieldErrors::default();
        let email = f.email("email", "  Not An Email ");
        f.name("first_name", "   ");
        assert_eq!(f.bio("bio", "  "), None);
        assert_eq!(f.bio("bio", " Hi "), Some("Hi".to_string()));
        f.bio("long_bio", &"x".repeat(BIO_MAX + 1));
        assert_eq!(email, "not an email");
        let err = f.check().unwrap_err();
        let fields = &err.details().unwrap()["fields"];
        assert_eq!(fields["email"][0], "invalid_email");
        assert_eq!(fields["first_name"][0], "required");
        assert_eq!(fields["long_bio"][0], "too_long");
        assert!(fields.get("bio").is_none());
    }
}
