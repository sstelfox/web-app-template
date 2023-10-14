use sqlx::SqlitePool;

pub(crate) async fn test_database() -> SqlitePool {
    SqlitePool::connect("sqlite::memory:").await.expect("db setup")
}
