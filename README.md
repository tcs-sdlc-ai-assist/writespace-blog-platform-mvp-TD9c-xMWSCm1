# WriteSpace

A clean, modern blogging platform built with Rust, Axum, and Tailwind CSS. Designed for writers who value simplicity and elegance.

## Tech Stack

- **Backend:** Rust with [Axum 0.8](https://github.com/tokio-rs/axum) web framework
- **Database:** [Neon](https://neon.tech/) serverless PostgreSQL via [sqlx](https://github.com/launchbadge/sqlx)
- **Authentication:** JWT-based auth with [jsonwebtoken](https://github.com/Keats/jsonwebtoken) and [bcrypt](https://github.com/Keats/rust-bcrypt) password hashing
- **Frontend:** Static HTML pages with [Tailwind CSS](https://tailwindcss.com/) (CDN) and vanilla JavaScript
- **Deployment:** [Vercel](https://vercel.com/) with `@vercel/rust` runtime via [vercel_runtime](https://crates.io/crates/vercel_runtime)

## Project Structure

```
writespace-rust/
├── api/
│   └── main.rs                 # Vercel serverless entry point (Axum router + vercel_runtime)
├── migrations/
│   └── 001_initial.sql         # Database schema (users, posts tables)
├── public/                     # Static frontend files served by Vercel
│   ├── index.html              # Landing page
│   ├── login.html              # Login page
│   ├── register.html           # Registration page
│   ├── blogs.html              # All posts listing (authenticated)
│   ├── blog.html               # Single post view
│   ├── write.html              # Create / edit post
│   ├── admin.html              # Admin dashboard
│   ├── users.html              # Admin user management
│   └── js/
│       └── app.js              # Shared JavaScript (API client, auth, nav, helpers)
├── src/
│   ├── lib.rs                  # Library crate root (re-exports all modules)
│   ├── config.rs               # Environment variable configuration
│   ├── db.rs                   # Database pool creation, migrations, admin seeding
│   ├── errors.rs               # AppError enum implementing IntoResponse
│   ├── models.rs               # Data models, DTOs, JWT claims, serde structs
│   ├── middleware/
│   │   ├── mod.rs              # Middleware module exports
│   │   └── auth.rs             # JwtService, AuthUser / OptionalAuthUser extractors
│   └── handlers/
│       ├── mod.rs              # Handler module exports
│       ├── auth.rs             # Login and registration handlers
│       ├── posts.rs            # CRUD handlers for blog posts
│       └── admin.rs            # Admin stats, user management handlers
├── tests/
│   └── handlers_test.rs        # Integration tests against a real database
├── Cargo.toml                  # Rust dependencies and binary configuration
├── vercel.json                 # Vercel build and routing configuration
├── .env.example                # Example environment variables
└── README.md                   # This file
```

## Features

- **User Authentication** — Register, login, and JWT-based session management
- **Role-Based Access Control** — Admin and regular user roles with permission checks
- **Blog Post CRUD** — Create, read, update, and delete posts with ownership enforcement
- **Admin Dashboard** — Platform statistics, user management, and recent post overview
- **Public Landing Page** — Displays the latest 3 posts to unauthenticated visitors
- **Responsive UI** — Tailwind CSS utility-first styling with mobile-friendly layouts
- **Default Admin Seeding** — Automatically creates an admin account on first startup

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain, 1.70+)
- A [Neon](https://neon.tech/) PostgreSQL database (or any PostgreSQL 14+ instance)
- [Vercel CLI](https://vercel.com/docs/cli) (optional, for deployment)

## Setup

### 1. Clone the repository

```bash
git clone <repository-url>
cd writespace-rust
```

### 2. Configure environment variables

Copy the example environment file and fill in your values:

```bash
cp .env.example .env
```

Edit `.env` with your configuration:

| Variable | Description | Example |
|---|---|---|
| `DATABASE_URL` | PostgreSQL connection string. `sslmode=require` is mandatory for Neon. | `postgresql://user:pass@ep-example.us-east-2.aws.neon.tech/writespace?sslmode=require` |
| `JWT_SECRET` | Secret key for signing JWT tokens. Use a strong random string (32+ chars). | `openssl rand -base64 64` |
| `DEFAULT_ADMIN_USERNAME` | Username for the auto-seeded admin account. | `admin` |
| `DEFAULT_ADMIN_PASSWORD` | Password for the auto-seeded admin account. Change before production. | `change-me-in-production` |
| `RUST_LOG` | Rust tracing log level configuration. | `writespace=info,tower_http=debug` |

### 3. Database setup

The application automatically runs migrations and seeds the default admin user on startup. No manual database setup is required beyond providing a valid `DATABASE_URL`.

The migration (`migrations/001_initial.sql`) creates:

- `users` table — id, display_name, username, password_hash, role, is_default_admin, created_at
- `posts` table — id, title, content, created_at, author_id (foreign key to users)
- Indexes on `posts.created_at` and `posts.author_id`

### 4. Local development

Build and run the application locally:

```bash
cargo build
cargo run --bin api
```

The server starts via `vercel_runtime`, which is designed for the Vercel serverless environment. For local testing, use the Vercel CLI:

```bash
vercel dev
```

This serves both the API routes (`/api/*`) and static files (`public/*`) locally.

### 5. Run tests

Integration tests require a running PostgreSQL database. Set `DATABASE_URL` in your environment or `.env` file, then run:

```bash
cargo test
```

Tests create and clean up their own data using unique usernames to avoid conflicts.

## API Reference

All API endpoints are prefixed with `/api`.

### Authentication

| Method | Endpoint | Auth | Description |
|---|---|---|---|
| `POST` | `/api/auth/login` | No | Login with username and password. Returns JWT token. |
| `POST` | `/api/auth/register` | No | Register a new user account. Returns JWT token. |

**Login request body:**
```json
{
  "username": "string",
  "password": "string"
}
```

**Register request body:**
```json
{
  "display_name": "string",
  "username": "string",
  "password": "string"
}
```

**Auth response:**
```json
{
  "token": "jwt-token-string",
  "user": {
    "id": "uuid",
    "display_name": "string",
    "username": "string",
    "role": "admin|user"
  }
}
```

### Posts

| Method | Endpoint | Auth | Description |
|---|---|---|---|
| `GET` | `/api/posts` | Optional | List posts. Unauthenticated: latest 3. Authenticated: all posts. |
| `GET` | `/api/posts/{id}` | Optional | Get a single post by ID. |
| `POST` | `/api/posts` | Required | Create a new post. |
| `PUT` | `/api/posts/{id}` | Required | Update a post. Owner or admin only. |
| `DELETE` | `/api/posts/{id}` | Required | Delete a post. Owner or admin only. |

**Create/Update request body:**
```json
{
  "title": "string (max 200 chars)",
  "content": "string"
}
```

**Post response:**
```json
{
  "id": "uuid",
  "title": "string",
  "content": "string",
  "created_at": "ISO 8601 datetime",
  "author": {
    "id": "uuid",
    "display_name": "string",
    "role": "admin|user"
  },
  "can_edit": true,
  "can_delete": true
}
```

### Admin

| Method | Endpoint | Auth | Description |
|---|---|---|---|
| `GET` | `/api/admin/stats` | Admin | Dashboard statistics (total posts, users, admins, recent posts). |
| `GET` | `/api/admin/users` | Admin | List all users. |
| `POST` | `/api/admin/users` | Admin | Create a new user with a specified role. |
| `DELETE` | `/api/admin/users/{id}` | Admin | Delete a user. Cannot delete self or default admin. |

**Create user request body:**
```json
{
  "display_name": "string",
  "username": "string",
  "password": "string",
  "role": "admin|user"
}
```

### Authentication Header

For protected endpoints, include the JWT token in the `Authorization` header:

```
Authorization: Bearer <token>
```

### Error Responses

All errors return a JSON body with an `error` field:

```json
{
  "error": "Description of the error"
}
```

| Status Code | Meaning |
|---|---|
| `400` | Bad Request — invalid input or validation failure |
| `401` | Unauthorized — missing or invalid authentication |
| `403` | Forbidden — insufficient permissions |
| `404` | Not Found — resource does not exist |
| `409` | Conflict — duplicate resource (e.g., username taken) |
| `500` | Internal Server Error — unexpected server failure |

## Deployment

### Vercel

The project is configured for deployment on Vercel using `@vercel/rust` for the API and `@vercel/static` for the frontend.

1. Install the Vercel CLI:

   ```bash
   npm i -g vercel
   ```

2. Link your project:

   ```bash
   vercel link
   ```

3. Set environment variables in the Vercel dashboard or via CLI:

   ```bash
   vercel env add DATABASE_URL
   vercel env add JWT_SECRET
   vercel env add DEFAULT_ADMIN_USERNAME
   vercel env add DEFAULT_ADMIN_PASSWORD
   ```

4. Deploy:

   ```bash
   vercel --prod
   ```

The `vercel.json` configuration routes `/api/*` requests to the Rust serverless function and all other requests to the `public/` static files.

### Routing

| Pattern | Destination |
|---|---|
| `/api/*` | `api/main.rs` (Rust serverless function) |
| `/*` | `public/*` (static HTML/JS/CSS) |

## Architecture Notes

- **Serverless Model:** Each API request spins up the Rust binary via `vercel_runtime`. The database pool, migrations, and admin seeding run on each cold start. Neon's serverless PostgreSQL handles connection pooling efficiently.
- **JWT Extension Middleware:** A custom `from_fn` middleware injects `JwtService` into request extensions on every request. The `AuthUser` and `OptionalAuthUser` extractors read from these extensions to authenticate requests.
- **State Management:** The Axum router uses a composite `AppState` with `FromRef` implementations so handlers can extract `State<PgPool>` and `State<JwtService>` independently.
- **Frontend:** All pages are static HTML files that use a shared `app.js` for API communication, authentication state, navigation rendering, and UI helpers. No build step is required for the frontend.

## License

Private