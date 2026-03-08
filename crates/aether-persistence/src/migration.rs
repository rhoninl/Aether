//! Database migration framework.
//!
//! Migrations are versioned SQL statements that are applied in order. Each migration
//! is recorded in a `migrations` table so it runs exactly once.

use crate::error::PersistenceError;
use crate::postgres::DatabaseClient;

/// A single database migration.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Unique, monotonically increasing version number.
    pub version: u32,
    /// Human-readable name for logging.
    pub name: String,
    /// SQL to execute (may contain multiple statements separated by `;`).
    pub sql: String,
}

impl Migration {
    /// Create a new migration.
    pub fn new(version: u32, name: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            sql: sql.into(),
        }
    }
}

/// Validates a list of migrations: versions must be unique and sorted ascending.
pub fn validate_migrations(migrations: &[Migration]) -> Result<(), PersistenceError> {
    if migrations.is_empty() {
        return Ok(());
    }

    for window in migrations.windows(2) {
        if window[1].version <= window[0].version {
            return Err(PersistenceError::MigrationError(format!(
                "migration versions must be strictly ascending: version {} is not greater than {}",
                window[1].version, window[0].version
            )));
        }
    }

    // Check for duplicate versions.
    let mut seen = std::collections::HashSet::new();
    for m in migrations {
        if !seen.insert(m.version) {
            return Err(PersistenceError::MigrationError(format!(
                "duplicate migration version: {}",
                m.version
            )));
        }
    }

    Ok(())
}

/// Returns the built-in migrations for the aether-persistence schema.
pub fn built_in_migrations() -> Vec<Migration> {
    vec![
        Migration::new(
            1,
            "create_world_snapshots",
            "CREATE TABLE IF NOT EXISTS world_snapshots (
                id          BIGSERIAL PRIMARY KEY,
                world_id    TEXT NOT NULL,
                tick        BIGINT NOT NULL,
                captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                kind        TEXT NOT NULL,
                actor_count INTEGER NOT NULL,
                data        BYTEA
            )",
        ),
        Migration::new(
            2,
            "create_wal_entries",
            "CREATE TABLE IF NOT EXISTS wal_entries (
                sequence    BIGSERIAL PRIMARY KEY,
                world_id    TEXT NOT NULL,
                key         TEXT NOT NULL,
                payload_crc32 BIGINT NOT NULL,
                timestamp_ms BIGINT NOT NULL,
                durability  TEXT NOT NULL
            )",
        ),
        Migration::new(
            3,
            "create_script_state",
            "CREATE TABLE IF NOT EXISTS script_state (
                world_id    TEXT NOT NULL,
                script_name TEXT NOT NULL,
                payload     BYTEA NOT NULL,
                updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (world_id, script_name)
            )",
        ),
        Migration::new(
            4,
            "create_migrations_table",
            "CREATE TABLE IF NOT EXISTS migrations (
                version     INTEGER PRIMARY KEY,
                name        TEXT NOT NULL,
                applied_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        ),
    ]
}

/// Run all pending migrations against a `DatabaseClient`.
///
/// This is the core migration runner. It:
/// 1. Ensures the migrations table exists
/// 2. Checks which versions have already been applied
/// 3. Applies any new migrations in order
pub async fn run_migrations(
    client: &dyn DatabaseClient,
    migrations: &[Migration],
) -> Result<u32, PersistenceError> {
    validate_migrations(migrations)?;

    // Ensure migrations tracking table exists (migration 4 creates it, but we
    // need it to track migrations including migration 4 itself).
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS migrations (
                version     INTEGER PRIMARY KEY,
                name        TEXT NOT NULL,
                applied_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
            &[],
        )
        .await?;

    let mut applied_count = 0u32;

    for migration in migrations {
        // Check if already applied (using a simple query).
        let already_applied = client
            .fetch_optional(
                &format!(
                    "SELECT version AS data FROM migrations WHERE version = {}",
                    migration.version
                ),
                &[],
            )
            .await?;

        if already_applied.is_some() {
            continue;
        }

        // Execute the migration SQL.
        client.execute(&migration.sql, &[]).await?;

        // Record that it was applied.
        client
            .execute(
                &format!(
                    "INSERT INTO migrations (version, name) VALUES ({}, '{}')",
                    migration.version,
                    migration.name.replace('\'', "''")
                ),
                &[],
            )
            .await?;

        applied_count += 1;
    }

    Ok(applied_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres::MockDatabaseClient;

    #[test]
    fn validate_empty_migrations_is_ok() {
        assert!(validate_migrations(&[]).is_ok());
    }

    #[test]
    fn validate_single_migration_is_ok() {
        let migrations = vec![Migration::new(1, "first", "SELECT 1")];
        assert!(validate_migrations(&migrations).is_ok());
    }

    #[test]
    fn validate_ascending_versions_is_ok() {
        let migrations = vec![
            Migration::new(1, "first", "SELECT 1"),
            Migration::new(2, "second", "SELECT 2"),
            Migration::new(5, "fifth", "SELECT 5"),
        ];
        assert!(validate_migrations(&migrations).is_ok());
    }

    #[test]
    fn validate_non_ascending_versions_fails() {
        let migrations = vec![
            Migration::new(2, "second", "SELECT 2"),
            Migration::new(1, "first", "SELECT 1"),
        ];
        let result = validate_migrations(&migrations);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("strictly ascending"));
    }

    #[test]
    fn validate_duplicate_versions_fails() {
        let migrations = vec![
            Migration::new(1, "first", "SELECT 1"),
            Migration::new(1, "also first", "SELECT 2"),
        ];
        // The ascending check catches duplicates too (1 is not > 1)
        assert!(validate_migrations(&migrations).is_err());
    }

    #[test]
    fn built_in_migrations_are_valid() {
        let migrations = built_in_migrations();
        assert!(validate_migrations(&migrations).is_ok());
        assert_eq!(migrations.len(), 4);
    }

    #[test]
    fn built_in_migration_versions_are_sequential() {
        let migrations = built_in_migrations();
        for (i, m) in migrations.iter().enumerate() {
            assert_eq!(m.version as usize, i + 1);
        }
    }

    #[test]
    fn migration_new_sets_fields() {
        let m = Migration::new(42, "test_migration", "CREATE TABLE test()");
        assert_eq!(m.version, 42);
        assert_eq!(m.name, "test_migration");
        assert_eq!(m.sql, "CREATE TABLE test()");
    }

    #[tokio::test]
    async fn run_migrations_with_mock_applies_all() {
        let client = MockDatabaseClient::healthy();
        let migrations = vec![
            Migration::new(1, "first", "CREATE TABLE a()"),
            Migration::new(2, "second", "CREATE TABLE b()"),
        ];
        let applied = run_migrations(&client, &migrations).await.unwrap();
        // Mock fetch_optional returns None (not applied), so all get applied.
        assert_eq!(applied, 2);
    }

    #[tokio::test]
    async fn run_migrations_with_mock_already_applied() {
        // When fetch_optional returns Some (already applied), migration is skipped.
        let client = MockDatabaseClient::healthy().with_fetch_data(vec![1]);
        let migrations = vec![Migration::new(1, "first", "CREATE TABLE a()")];
        let applied = run_migrations(&client, &migrations).await.unwrap();
        assert_eq!(applied, 0);
    }

    #[tokio::test]
    async fn run_migrations_invalid_order_fails() {
        let client = MockDatabaseClient::healthy();
        let migrations = vec![
            Migration::new(2, "second", "SELECT 2"),
            Migration::new(1, "first", "SELECT 1"),
        ];
        let result = run_migrations(&client, &migrations).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn run_migrations_empty_list_applies_zero() {
        let client = MockDatabaseClient::healthy();
        let applied = run_migrations(&client, &[]).await.unwrap();
        assert_eq!(applied, 0);
    }

    #[test]
    fn migration_is_clone_and_debug() {
        let m = Migration::new(1, "test", "SELECT 1");
        let cloned = m.clone();
        assert_eq!(cloned.version, m.version);
        let debug = format!("{m:?}");
        assert!(debug.contains("Migration"));
    }
}
