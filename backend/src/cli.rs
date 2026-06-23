//! `admin` CLI subcommands for bootstrapping and recovering super-admin
//! accounts without direct database access.
//!
//!   wqn-backend admin create --email admin@example.com
//!   wqn-backend admin reset-password --email admin@example.com
//!   wqn-backend admin create --email admin@example.com --password-stdin < pw
//!
//! In Docker: `docker compose exec -it backend wqn-backend admin create ...`
//! Locally:   `cargo run -- admin create ...`
//!
//! Security properties:
//!   - Passwords are never accepted as a `--password` argument (that would leak
//!     via `ps` and shell history). Interactive mode reads from the TTY with
//!     echo disabled; `--password-stdin` reads a single line from stdin for
//!     automation and skips the confirmation prompt.
//!   - `create` is idempotent: a missing user is created as `super_admin`, an
//!     existing non-super-admin is promoted, an existing super-admin is a no-op.
//!     The password is only requested when a new account actually has to be
//!     created, so promoting an existing user needs no credential entry.
//!   - `reset-password` bumps `token_version`, invalidating every existing
//!     session so the old (possibly leaked) credential cannot be reused.
//!   - Neither command consults the self-service `user_registration` setting;
//!     bootstrap always works even when public registration is closed.

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result, anyhow, bail};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::AppConfig,
    services::password::{hash_password, normalize_email, validate_password},
};

const USAGE: &str = "\
Usage:
  wqn-backend admin create --email <email> [--password-stdin]
  wqn-backend admin reset-password --email <email> [--password-stdin]

Options:
  --email <email>      Admin email (matched case-insensitively, as lower(email)).
  --password-stdin     Read the password from stdin instead of prompting on the
                       TTY. Intended for automation; skips the confirmation prompt.

The same .env / environment variables as the server are required (DATABASE_URL,
AUTH_JWT_SECRET, ...). Migrations run automatically before any change.";

/// Entry point for the `admin` subcommand. `args` is everything after `admin`
/// (i.e. `["create", "--email", ...]` or `["reset-password", ...]`).
pub async fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("create") => create(&args[1..]).await,
        Some("reset-password") => reset_password(&args[1..]).await,
        Some("help") | Some("-h") | Some("--help") => {
            println!("{USAGE}");
            Ok(())
        }
        other => {
            eprintln!(
                "error: unknown admin subcommand: {}\n",
                other.unwrap_or("(none)")
            );
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

async fn create(args: &[String]) -> Result<()> {
    let opts = parse_args(args, "create")?;
    let email = normalize_email(&opts.email)?;

    let config = AppConfig::from_env().context("failed to load configuration")?;
    let pool = crate::connect_and_migrate(&config).await?;

    match find_user_by_email(&pool, &email).await? {
        Some((user_id, role)) => {
            if role == "super_admin" {
                println!("{email} is already a super_admin (id={user_id}). Nothing changed.");
                return Ok(());
            }
            if !opts.password_stdin {
                confirm(&format!(
                    "Promote existing user {email} (id={user_id}) to super_admin?"
                ))?;
            }
            promote(&pool, user_id).await?;
            println!("Promoted {email} (id={user_id}) to super_admin.");
            Ok(())
        }
        None => {
            // Only ask for a password when we actually need to create the account.
            let password = read_password(opts.password_stdin)?;
            validate_password(&password)?;
            if !opts.password_stdin {
                confirm(&format!("Create super_admin {email}?"))?;
            }
            let password_hash = hash_password(&password)?;
            let user_id = create_super_admin(&pool, &email, &password_hash).await?;
            println!("Created super_admin {email} (id={user_id}).");
            Ok(())
        }
    }
}

async fn reset_password(args: &[String]) -> Result<()> {
    let opts = parse_args(args, "reset-password")?;
    let email = normalize_email(&opts.email)?;
    let password = read_password(opts.password_stdin)?;
    validate_password(&password)?;

    let config = AppConfig::from_env().context("failed to load configuration")?;
    let pool = crate::connect_and_migrate(&config).await?;

    let (user_id, role) = match find_user_by_email(&pool, &email).await? {
        Some(user) => user,
        None => bail!("no user found with email {email}"),
    };
    if role != "super_admin" {
        bail!(
            "{email} is not a super_admin (current role: {role}). \
             `admin reset-password` only resets super-admin accounts."
        );
    }

    if !opts.password_stdin {
        confirm(&format!("Reset password for super_admin {email} (id={user_id})?"))?;
    }

    let password_hash = hash_password(&password)?;
    // Bump token_version so every existing session JWT is invalidated; the admin
    // must re-authenticate with the new password.
    let result = sqlx::query(
        "update app_users \
         set password_hash = $2, token_version = token_version + 1, updated_at = now() \
         where id = $1",
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(&pool)
    .await
    .context("failed to update password")?;

    if result.rows_affected() == 0 {
        bail!("password was not updated; the account may have been removed concurrently");
    }

    println!("Password reset for super_admin {email} (id={user_id}).");
    println!("All existing sessions have been invalidated; log in with the new password.");
    Ok(())
}

struct AdminOptions {
    email: String,
    password_stdin: bool,
}

fn parse_args(args: &[String], subcommand: &str) -> Result<AdminOptions> {
    let mut email: Option<String> = None;
    let mut password_stdin = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--email" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| anyhow!("--email requires a value\n\n{USAGE}"))?;
                email = Some(value.clone());
            }
            "--password-stdin" => password_stdin = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("unknown argument for `admin {subcommand}`: {other:?}\n\n{USAGE}"),
        }
        i += 1;
    }

    let email = email
        .ok_or_else(|| anyhow!("--email is required for `admin {subcommand}`\n\n{USAGE}"))?;
    Ok(AdminOptions {
        email,
        password_stdin,
    })
}

/// Look up a user by email (case-insensitive) and return their id and role. A
/// user with no profile row reports the default role `user`.
async fn find_user_by_email(pool: &PgPool, email: &str) -> Result<Option<(Uuid, String)>> {
    sqlx::query_as::<_, (Uuid, String)>(
        "select au.id, coalesce(up.user_role, 'user') as role \
         from app_users au \
         left join user_profiles up on up.id = au.id \
         where lower(au.email) = lower($1)",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .context("failed to look up user")
}

/// Set `user_role = 'super_admin'`, creating the profile row if it is missing.
/// Existing profile data (username, timezone, ...) is preserved.
async fn promote(pool: &PgPool, user_id: Uuid) -> Result<()> {
    sqlx::query(
        "insert into user_profiles (id, user_role) values ($1, 'super_admin') \
         on conflict (id) do update set user_role = 'super_admin', updated_at = now()",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .context("failed to promote user")?;
    Ok(())
}

/// Create a brand-new `super_admin` account (app_users + user_profiles) in one
/// transaction. Returns the new user id.
async fn create_super_admin(pool: &PgPool, email: &str, password_hash: &str) -> Result<Uuid> {
    let user_id = Uuid::new_v4();
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    let insert_result = sqlx::query(
        "insert into app_users (id, email, password_hash) values ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(email)
    .bind(password_hash)
    .execute(&mut *tx)
    .await;

    if let Err(err) = insert_result {
        return Err(if is_unique_violation(&err) {
            anyhow!("email {email} is already registered")
        } else {
            anyhow!("failed to insert app_users: {err}")
        });
    }

    sqlx::query("insert into user_profiles (id, user_role) values ($1, 'super_admin')")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("failed to insert user_profiles")?;

    tx.commit().await.context("failed to commit transaction")?;
    Ok(user_id)
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

fn read_password(from_stdin: bool) -> Result<String> {
    let password = if from_stdin {
        let mut line = String::new();
        io::stdin()
            .lock()
            .read_line(&mut line)
            .context("failed to read password from stdin")?;
        // Drop the line terminator only; passwords may contain other whitespace.
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        line
    } else {
        let first = rpassword::prompt_password("Password: ")
            .context("failed to read password (no TTY available? pass --password-stdin)")?;
        println!();
        let confirmation = rpassword::prompt_password("Confirm password: ")
            .context("failed to read password confirmation")?;
        println!();
        if first != confirmation {
            bail!("passwords do not match");
        }
        first
    };
    Ok(password)
}

fn confirm(prompt: &str) -> Result<()> {
    print!("{prompt} [y/N] ");
    io::stdout().flush().ok();
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .context("failed to read confirmation")?;
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        bail!("aborted");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    #[test]
    fn parse_args_reads_email_and_stdin_flag() {
        let args = vec![
            "--email".to_owned(),
            "Admin@Example.com".to_owned(),
            "--password-stdin".to_owned(),
        ];
        let opts = parse_args(&args, "create").expect("valid args");
        assert_eq!(opts.email, "Admin@Example.com");
        assert!(opts.password_stdin);
    }

    #[test]
    fn parse_args_defaults_stdin_flag_to_false() {
        let args = vec!["--email".to_owned(), "a@b.c".to_owned()];
        let opts = parse_args(&args, "create").expect("valid args");
        assert_eq!(opts.email, "a@b.c");
        assert!(!opts.password_stdin);
    }

    #[test]
    fn parse_args_requires_email() {
        let args = vec!["--password-stdin".to_owned()];
        assert!(parse_args(&args, "create").is_err());
    }

    #[test]
    fn parse_args_rejects_unknown_flag() {
        let args = vec![
            "--email".to_owned(),
            "a@b.c".to_owned(),
            "--bogus".to_owned(),
        ];
        assert!(parse_args(&args, "create").is_err());
    }

    #[test]
    fn parse_args_email_without_value_fails() {
        let args = vec!["--email".to_owned()];
        assert!(parse_args(&args, "create").is_err());
    }
}
