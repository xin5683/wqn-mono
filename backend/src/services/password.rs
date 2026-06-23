//! Email/password validation and Argon2 hashing, shared by the self-service
//! auth routes and the `admin` CLI. Centralising these keeps the hashing
//! parameters and password policy in exactly one place.

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};

use crate::error::{AppError, AppResult};

/// Trim, lowercase, and sanity-check an email address. The lowercasing matches
/// the `app_users_lower_email_idx` assumption that emails compare case-insensitively.
pub(crate) fn normalize_email(email: &str) -> AppResult<String> {
    let trimmed = email.trim().to_ascii_lowercase();
    if !trimmed.contains('@') || trimmed.len() > 320 {
        return Err(AppError::BadRequest("Invalid email".to_owned()));
    }
    Ok(trimmed)
}

/// Minimum password policy. Deliberately light: the credential is chosen by an
/// operator (CLI) or a self-registering user; depth of policy lives elsewhere.
pub(crate) fn validate_password(password: &str) -> AppResult<()> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_owned(),
        ));
    }
    Ok(())
}

/// Argon2id hash with a fresh random salt. The returned string embeds the
/// algorithm parameters and salt, so `verify_password` needs no extra state.
pub(crate) fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| AppError::Internal(format!("Password hashing failed: {err}")))
}

pub(crate) fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|err| AppError::Internal(format!("Invalid password hash: {err}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::{hash_password, normalize_email, validate_password, verify_password};

    #[test]
    fn normalize_email_lowercases_and_validates() {
        assert_eq!(normalize_email("  Admin@Example.com  ").unwrap(), "admin@example.com");
        assert!(normalize_email("no-at-sign").is_err());
        assert!(normalize_email(&"x".repeat(321)).is_err());
    }

    #[test]
    fn validate_password_requires_eight_chars() {
        assert!(validate_password("short").is_err());
        assert!(validate_password("longenough").is_ok());
    }

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }
}
