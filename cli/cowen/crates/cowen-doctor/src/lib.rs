pub mod task;

pub use task::*;
use anyhow::Result;

pub async fn run_all_diagnostics(ctx: &DoctorContext) -> Result<Vec<DiagnosticResult>> {
    let mut set = tokio::task::JoinSet::new();
    
    for reg in inventory::iter::<DiagnosticRegistration> {
        let task = (reg.builder)();
        let ctx_clone = ctx.clone();
        set.spawn(async move {
            task.run(&ctx_clone).await
        });
    }

    let mut results = Vec::new();
    while let Some(res) = set.join_next().await {
        if let Ok(Ok(diagnostic_res)) = res {
            results.push(diagnostic_res);
        } else if let Ok(Err(e)) = res {
            // Task failed internally, could wrap it or ignore
            tracing::error!("Diagnostic task failed: {}", e);
        }
    }

    Ok(results)
}
