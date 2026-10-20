use crate::error::{AppError, AppResult, FieldError};

#[derive(Default)]
pub struct Validator {
    errors: Vec<FieldError>,
}

impl Validator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check(&mut self, ok: bool, field: &str, message: &str) -> &mut Self {
        if !ok {
            self.errors.push(FieldError {
                field: field.to_owned(),
                message: message.to_owned(),
            });
        }
        self
    }

    pub fn required(&mut self, field: &str, value: &str, max_chars: usize) -> &mut Self {
        if value.is_empty() {
            self.check(false, field, "is required")
        } else {
            self.max_len(field, value, max_chars)
        }
    }

    pub fn optional(&mut self, field: &str, value: Option<&str>, max_chars: usize) -> &mut Self {
        match value {
            Some(value) => self.max_len(field, value, max_chars),
            None => self,
        }
    }

    pub fn email(&mut self, field: &str, value: &str) -> &mut Self {
        self.check(
            is_plausible_email(value),
            field,
            "must be a valid email address",
        )
    }

    fn max_len(&mut self, field: &str, value: &str, max_chars: usize) -> &mut Self {
        let message = format!("must be at most {max_chars} characters");
        self.check(value.chars().count() <= max_chars, field, &message)
    }

    pub fn finish(&mut self) -> AppResult<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(std::mem::take(&mut self.errors)))
        }
    }
}

pub fn clean(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

pub fn normalize_email(value: &str) -> String {
    value.trim().to_lowercase()
}

fn is_plausible_email(value: &str) -> bool {
    if value.len() > 254 || value.chars().any(char::is_whitespace) {
        return false;
    }
    match value.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && !domain.contains('@')
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        }
        None => false,
    }
}

pub fn normalize_domain(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let host = lower
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    host.split('/').next().unwrap_or(host).to_owned()
}

pub fn like_pattern(search: &str) -> String {
    let mut escaped = String::with_capacity(search.len() + 2);
    escaped.push('%');
    for ch in search.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped.push('%');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_lose_scheme_www_and_path() {
        assert_eq!(normalize_domain(" HTTPS://www.Acme.io/about "), "acme.io");
        assert_eq!(normalize_domain("harbor.example"), "harbor.example");
    }

    #[test]
    fn accepts_ordinary_emails() {
        assert!(is_plausible_email("ana@example.com"));
        assert!(is_plausible_email("first.last+tag@sub.example.co"));
    }

    #[test]
    fn rejects_malformed_emails() {
        for bad in [
            "",
            "plain",
            "@example.com",
            "a@b",
            "a@@b.com",
            "a b@c.com",
            "a@.com",
            "a@b.com.",
        ] {
            assert!(!is_plausible_email(bad), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn clean_drops_blank_values() {
        assert_eq!(clean(Some("  ".into())), None);
        assert_eq!(clean(Some(" acme ".into())), Some("acme".into()));
        assert_eq!(clean(None), None);
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("acme"), "%acme%");
        assert_eq!(like_pattern("50%_off\\"), "%50\\%\\_off\\\\%");
    }

    #[test]
    fn validator_collects_every_error() {
        let result = Validator::new()
            .required("name", "", 10)
            .required("title", "this is far too long", 5)
            .email("email", "nope")
            .finish();
        match result {
            Err(AppError::Validation(errors)) => {
                let fields: Vec<_> = errors.iter().map(|e| e.field.as_str()).collect();
                assert_eq!(fields, ["name", "title", "email"]);
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn length_limit_counts_characters_not_bytes() {
        assert!(Validator::new()
            .required("name", "Жанна", 5)
            .finish()
            .is_ok());
    }
}
