use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ResetTask: Send + Sync {
    /// Name of the reset task module
    fn name(&self) -> &'static str;
    
    /// Description of what will be reset
    fn description(&self) -> &'static str;
    
    /// Dry run: Describe what would be deleted without actually deleting
    async fn dry_run(&self) -> Result<Vec<String>>;
    
    /// Execute the reset logic
    async fn execute(&self) -> Result<()>;
}

pub struct ResetEngine {
    tasks: Vec<Box<dyn ResetTask>>,
}

impl ResetEngine {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn with(mut self, task: Box<dyn ResetTask>) -> Self {
        self.tasks.push(task);
        self
    }

    pub async fn run(&self, dry_run: bool) -> Result<()> {
        if dry_run {
            println!("🔍 [DRY RUN] Reset Execution Plan:");
            for task in &self.tasks {
                println!("\n  📦 Module: {}", task.name());
                println!("  ℹ️  {}", task.description());
                let actions = task.dry_run().await?;
                if actions.is_empty() {
                    println!("      - No actions to perform.");
                } else {
                    for action in actions {
                        println!("      - {}", action);
                    }
                }
            }
            println!("\n💡 This is a dry run. No actual changes were made.");
            return Ok(());
        }

        println!("⚠️  [DANGER] Executing System Reset...");
        for task in &self.tasks {
            println!("  🔄 Resetting {}...", task.name());
            if let Err(e) = task.execute().await {
                eprintln!("  ❌ Failed to reset {}: {}", task.name(), e);
            } else {
                println!("  ✅ Successfully reset {}.", task.name());
            }
        }
        println!("\n✨ System reset completed.");
        Ok(())
    }
}
