# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

CloudVault is a lightweight private cloud storage server: a Rust/Axum + PostgreSQL
JSON API (`src/`) plus a static vanilla-JS frontend (`web/`) that calls it. Users
register/login, upload files to local disk, organize them into folders, and create
public share links.

## Commands

```bash
cargo build                      # build the server binary (cloudvault-server)
cargo run                        # run the server (reads .env, binds CV__SERVER__HOST:PORT, default 0.0.0.0:8080)
cargo test --test models_test    # pure unit tests — no DB or server needed
cargo test --test integration_test   # integration tests — REQUIRE a running server + DB (see below)
cargo test test_user_new         # run a single test by name
```

Frontend: open `web/index.html` in a browser (or serve `web/` statically); it
targets the API at `http://localhost:8080`.

## Configuration

Config is loaded in [src/config.rs](src/config.rs) via the `config` crate: hardcoded
defaults, then overridden by environment variables (and `.env` through `dotenvy`).
Env vars use the prefix `CV` and `__` as the nesting separator — e.g.
`CV__DATABASE__URL`, `CV__SERVER__PORT`, `CV__JWT__SECRET`, `CV__STORAGE__LOCAL_PATH`.
See [.env.example](.env.example). There are no `config/` files despite the prefix;
the directory is empty and config is env-driven.

## Architecture

Request flow: `axum::serve` → CORS layer → `jwt_auth_layer` middleware → merged
route routers → handler. Wiring lives in [src/main.rs](src/main.rs); note the layer
order means the JWT middleware runs as the outermost wrapper for every route.

- `AppState` (`{ config, db }`) is the shared state threaded through every handler
  via `State<AppState>`. It is defined in BOTH [src/lib.rs](src/lib.rs) and
  [src/main.rs](src/main.rs) — the binary uses its own copy, the test crate uses the
  `lib` copy. Keep the two definitions in sync when changing state.
- Auth: JWT (HS256) signed with `jwt.secret`. [src/middleware.rs](src/middleware.rs)
  validates the `Bearer` token, then injects the user id (the `sub` claim) into request
  extensions. Handlers read it with `Extension<String>` — that String IS the
  authenticated user id. Public paths are allowlisted by prefix inside the middleware
  (`/api/auth/login`, `/api/auth/register`, `/api/shares/public`, `/debug`, `/health`)
  and `OPTIONS` is always allowed; everything else requires a valid token.
- Routes: each file in [src/routes/](src/routes/) exposes a `routes() -> Router<AppState>`
  that is `.merge`d in main. Handlers run SQL inline with `sqlx::query`/`query_as`
  against `state.db.pool()` — there is no repository/service layer.
- Data model ([src/models.rs](src/models.rs)): `User`, `File`, `Share` are `FromRow`
  structs mirroring the tables. Each has a paired `*Response` struct and a `From` impl
  that strips sensitive/internal fields (e.g. `UserResponse` drops `password_hash`,
  `FileResponse` drops `storage_path`/`storage_type`). Return `*Response` types from
  handlers, never the raw row structs.

## Domain specifics

- Files and folders share one `files` table; `is_folder` distinguishes them and
  `parent_id` is a self-referencing FK forming the tree. Folder renames/moves must
  also rewrite the `path` of all descendants (see `update_file` in
  [src/routes/files.rs](src/routes/files.rs)). `parent_id IS NOT DISTINCT FROM $n` is
  used so a NULL parent (root) compares correctly in duplicate-name checks.
- Uploads ([src/routes/files.rs](src/routes/files.rs) `upload_file`) take raw request
  body + `?filename=` query param (not multipart). Bytes are written to
  `{storage.local_path}/{user_id[..8]}/{uuid}`; that relative path is stored in
  `storage_path`. Folders have no `storage_path`. Deleting a file best-effort removes
  the disk file then the DB row (children cascade via FK).
- IDs are app-generated `VARCHAR(36)` UUID strings (not DB-native uuid), set in the
  model `new()` constructors. Share tokens are dash-stripped UUIDs.
- Passwords are hashed with Argon2 (the `bcrypt` dep is present but unused).

## Database & migrations

PostgreSQL via `sqlx` with the runtime query API (`query`/`query_as`), so queries are
NOT checked at compile time and no `DATABASE_URL`/`sqlx-cli` is needed to build. The
schema in [src/db/migrations/001_initial_schema.sql](src/db/migrations/001_initial_schema.sql)
is NOT applied automatically — apply it manually to a running Postgres before the
server or integration tests will work.

## Tests

- `models_test.rs` is pure unit tests (constructors, `From` conversions) — no DB.
- `integration_test.rs` hits a server over HTTP at `http://localhost:8080`, so it
  needs both the DB schema applied AND `cargo run` already running in another shell.
  `tests/common/mod.rs` (`TestSetup`) connects to the same DB to seed a user + JWT and
  clean up afterward; cleanup is manual via `setup.cleanup()`, not `Drop`.

## Known rough edges

These are intentionally documented so you don't mistake them for the target state:
`main.rs` has `/debug/*` routes and the JWT middleware prints `[JWT DEBUG]` lines to
stdout; CORS is wide open (`Any` origin/headers/methods); `list_users` returns all
users with no admin check. Treat these as dev-only — flag them before relying on them
in anything user-facing.
