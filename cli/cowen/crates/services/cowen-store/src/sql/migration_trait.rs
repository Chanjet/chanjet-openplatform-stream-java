use async_trait::async_trait;
use cowen_common::CowenResult;

#[async_trait]
pub trait SchemaMigration: Send + Sync {
    /// Get the current migration version
    async fn get_current_version(&self) -> CowenResult<u32>;

    /// Apply a raw SQL statement
    async fn apply_sql(&self, sql: &str) -> CowenResult<()>;

    /// Set the current version
    async fn set_version(&self, version: u32) -> CowenResult<()>;

    /// Returns the ordered list of migrations to apply: (Version, SQL)
    fn get_migrations(&self) -> Vec<(u32, &'static str)>;

    /// Execute the migration process
    async fn run_migration(&self) -> CowenResult<()> {
        // Try to get current version, if table doesn't exist, it should return 0 or be initialized
        let current_version = self.get_current_version().await.unwrap_or(0);

        for (version, sql) in self.get_migrations() {
            if current_version < version {
                tracing::info!(target: "sys", "Applying schema migration version {}...", version);
                self.apply_sql(sql).await?;
                self.set_version(version).await?;
                tracing::info!(target: "sys", "Migration version {} applied successfully.", version);
            }
        }

        Ok(())
    }
}
