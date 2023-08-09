# Web App Template

* Rust
* Axum
* Either Sqlite or Postgres

## Development Environment

```
cargo install sqlx-cli --features postgres,sqlite
podman run -t -d -p 5432:5432 -e POSTGRES_PASSWORD=test_password docker.io/library/postgres:alpine
```
