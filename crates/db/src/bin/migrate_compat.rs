//! Explicit database migration tool; never defaults to an instance database.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")?;
    let pool = sqlx::SqlitePool::connect(&url).await?;
    let result = db::run_migrations(&pool).await;
    pool.close().await;
    result?;
    Ok(())
}
