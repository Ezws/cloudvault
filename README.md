# CloudVault

CloudVault is a lightweight private cloud storage app. It provides a Rust/Axum JSON API backed by PostgreSQL and a static vanilla JavaScript web UI for managing personal files, folders, shares, transfers, and video playback.

## What It Can Do

- User registration and login with JWT authentication.
- First registered user becomes an administrator.
- File and folder management:
  - create folders
  - upload files
  - download files with the original filename
  - rename, move, and delete files or folders
  - browse nested folders with breadcrumbs
- Public share links for files.
- Admin user management.
- macOS/Finder-style web interface.
- Transfer center:
  - upload and download records
  - progress bars
  - transfer speed display
  - pause, resume, and cancel for active transfers
  - resumable chunked uploads during the current browser session
  - ranged chunk downloads during the current browser session
- Video playback:
  - double-click common video files to open the built-in player
  - supports browser-playable formats such as MP4/WebM/MOV, and attempts playback for common video extensions such as MKV/AVI when the browser can decode them
  - uses HTTP Range requests so seeking works for large videos

## Tech Stack

- Backend: Rust, Axum, Tokio
- Database: PostgreSQL via SQLx
- Authentication: JWT, Argon2 password hashing
- Storage: local filesystem
- Frontend: static HTML, CSS, and vanilla JavaScript

## Project Layout

```text
.
├── Cargo.toml
├── src/
│   ├── main.rs              # server startup, router, CORS
│   ├── config.rs            # environment-driven configuration
│   ├── db.rs                # SQLx pool wrapper
│   ├── middleware.rs        # JWT middleware
│   ├── models.rs            # database models and response DTOs
│   └── routes/
│       ├── auth.rs
│       ├── files.rs
│       ├── shares.rs
│       └── users.rs
├── src/db/migrations/       # PostgreSQL schema SQL
├── tests/
└── web/
    ├── index.html
    ├── app.js
    └── styles.css
```

## Requirements

- Rust toolchain
- PostgreSQL
- A static file server for `web/` during development, for example Python's built-in HTTP server

## Configuration

Copy the example environment file and edit it:

```bash
cp .env.example .env
```

Important settings:

```text
CV__DATABASE__URL=postgres://cloudvault:password@localhost:5432/cloudvault
CV__SERVER__HOST=0.0.0.0
CV__SERVER__PORT=8080
CV__JWT__SECRET=change-this-in-production
CV__JWT__EXPIRATION_HOURS=24
CV__STORAGE__LOCAL_PATH=./storage
CV__CORS__ALLOWED_ORIGINS=http://localhost:8081,http://127.0.0.1:8081
```

Configuration is loaded from defaults, `.env`, and environment variables with the `CV__` prefix.

## Database Setup

Create a PostgreSQL database and apply the schema manually:

```bash
psql "$CV__DATABASE__URL" -f src/db/migrations/001_initial_schema.sql
psql "$CV__DATABASE__URL" -f src/db/migrations/002_add_admin_role.sql
```

The migrations are not applied automatically by the server.

## Start The Backend

```bash
cargo run
```

By default, the API listens on:

```text
http://localhost:8080
```

Health check:

```bash
curl http://localhost:8080/health
```

Expected response:

```text
OK
```

## Start The Frontend

In another terminal:

```bash
cd web
python3 -m http.server 8081 --bind 0.0.0.0
```

Then open:

```text
http://localhost:8081
```

The frontend defaults to API requests at:

```text
http://<current-host>:8080
```

You can change the API URL from the settings button in the UI.

## Development Commands

```bash
cargo build
cargo check
cargo test --test models_test
cargo test --test integration_test
```

Notes:

- `models_test` does not require a running database.
- `integration_test` requires PostgreSQL, the schema, and a running server at `http://localhost:8080`.

## API Overview

Authentication:

- `POST /api/auth/register`
- `POST /api/auth/login`
- `GET /api/auth/me`

Files:

- `GET /api/files`
- `POST /api/files`
- `POST /api/files/upload`
- `POST /api/files/uploads/init`
- `GET /api/files/uploads/{upload_id}/status`
- `POST /api/files/uploads/{upload_id}/chunk`
- `POST /api/files/uploads/{upload_id}/complete`
- `GET /api/files/{id}`
- `PATCH /api/files/{id}`
- `DELETE /api/files/{id}`
- `GET /api/files/{id}/download`

Shares:

- `POST /api/shares`
- `GET /api/shares`
- `DELETE /api/shares/{id}`
- `GET /api/shares/public/{token}`

Users:

- `GET /api/users`
- `GET /api/users/{id}`
- `PATCH /api/users/{id}`
- `DELETE /api/users/{id}`

## Transfer And Video Notes

- Upload pause/resume works within the current browser session. If the page is refreshed, the browser no longer keeps the selected local `File` handle, so continuing an unfinished upload requires selecting the file again in a future enhancement.
- Download pause/resume works within the current browser session by using HTTP Range requests.
- The video player uses the same authenticated download endpoint with Range support. The browser must support the video's codec/container to play it directly.
- Query-token access is accepted only for file download/playback URLs so that the native `<video>` element can fetch protected media.

## Production Notes

- Use a strong `CV__JWT__SECRET`.
- Restrict `CV__CORS__ALLOWED_ORIGINS` to trusted frontend origins.
- Serve the frontend and API through HTTPS.
- Put the backend behind a process manager such as systemd.
- Back up PostgreSQL and the configured storage directory.
