# Deployment Guide

## Overview

WritSpace is a Rust + Axum blog platform deployed as a Vercel Serverless Function with a Neon PostgreSQL database. The frontend is served as static files from the `public/` directory.

---

## Prerequisites

- [Vercel CLI](https://vercel.com/docs/cli) installed (`npm i -g vercel`)
- A [Vercel](https://vercel.com) account
- A [Neon](https://neon.tech) PostgreSQL database
- Rust toolchain installed (for local development)
- Git repository connected to Vercel for automatic deployments

---

## Neon PostgreSQL Setup

### 1. Create a Neon Project

1. Sign in at [https://console.neon.tech](https://console.neon.tech)
2. Click **New Project**
3. Choose a region close to your Vercel deployment region (e.g., `us-east-2` for AWS)
4. Name your project (e.g., `writespace`)
5. Copy the connection string from the dashboard

### 2. Connection String Format

Your connection string must include `sslmode=require` for Neon:

```
postgresql://<user>:<password>@<endpoint>.neon.tech/<database>?sslmode=require
```

Example:

```
postgresql://writespace_owner:AbCdEfGh@ep-cool-rain-123456.us-east-2.aws.neon.tech/writespace?sslmode=require
```

> **Important:** The `sslmode=require` parameter is mandatory. Connections without SSL will be rejected by Neon.

### 3. Database Initialization

Migrations run automatically on each cold start of the serverless function. The migration file at `migrations/001_initial.sql` creates:

- `users` table with UUID primary keys, roles, and unique username constraint
- `posts` table with foreign key to `users` and cascade delete
- Indexes on `posts.created_at` and `posts.author_id`

The migration is idempotent — if tables already exist, the migration is skipped without error.

### 4. Default Admin Seeding

On startup, the application checks for a user matching `DEFAULT_ADMIN_USERNAME`. If no such user exists, it creates one with:

- **Username:** value of `DEFAULT_ADMIN_USERNAME` (default: `admin`)
- **Password:** value of `DEFAULT_ADMIN_PASSWORD` (default: `change-me-in-production`)
- **Role:** `admin`
- **is_default_admin:** `true` (prevents deletion via the admin UI)

> **Security:** Always change `DEFAULT_ADMIN_PASSWORD` before deploying to production.

---

## Vercel Deployment

### 1. Project Structure

The `vercel.json` configuration defines two build targets:

```json
{
  "builds": [
    {
      "src": "api/main.rs",
      "use": "@vercel/rust"
    },
    {
      "src": "public/**",
      "use": "@vercel/static"
    }
  ],
  "routes": [
    {
      "src": "/api/(.*)",
      "dest": "api/main.rs"
    },
    {
      "src": "/(.*)",
      "dest": "public/$1"
    }
  ]
}
```

- All `/api/*` requests are routed to the Rust serverless function
- All other requests are served from the `public/` directory as static files

### 2. Environment Variables

Set the following environment variables in the Vercel dashboard under **Settings → Environment Variables**:

| Variable | Required | Description | Example |
|---|---|---|---|
| `DATABASE_URL` | Yes | Neon PostgreSQL connection string with `sslmode=require` | `postgresql://user:pass@host/db?sslmode=require` |
| `JWT_SECRET` | Yes | Secret key for signing JWT tokens (min 32 characters) | Output of `openssl rand -base64 64` |
| `DEFAULT_ADMIN_USERNAME` | No | Username for the seeded admin account (default: `admin`) | `admin` |
| `DEFAULT_ADMIN_PASSWORD` | No | Password for the seeded admin account | `my-secure-admin-password` |
| `RUST_LOG` | No | Log level configuration | `writespace=info,tower_http=debug` |

To set environment variables via the Vercel CLI:

```bash
vercel env add DATABASE_URL
vercel env add JWT_SECRET
vercel env add DEFAULT_ADMIN_PASSWORD
```

> **Tip:** Set variables for all environments (Production, Preview, Development) or scope them individually as needed.

### 3. Deploy

#### Automatic Deployment (Recommended)

1. Connect your Git repository to Vercel via the dashboard
2. Every push to the `main` branch triggers a production deployment
3. Pull request branches trigger preview deployments automatically

#### Manual Deployment

```bash
# Preview deployment
vercel

# Production deployment
vercel --prod
```

### 4. Domain Configuration

1. Go to **Settings → Domains** in the Vercel dashboard
2. Add your custom domain (e.g., `writespace.example.com`)
3. Configure DNS records as instructed by Vercel:
   - **CNAME** record pointing to `cname.vercel-dns.com` for subdomains
   - **A** record pointing to `76.76.21.21` for apex domains
4. SSL certificates are provisioned automatically by Vercel

---

## CI/CD Notes

### Automatic Deployments

When your Git repository is connected to Vercel:

- **Push to `main`** → Production deployment
- **Push to any other branch** → Preview deployment
- **Pull request opened** → Preview deployment with a unique URL

### Build Process

The `@vercel/rust` builder compiles the Rust binary during deployment. Build times for Rust projects are longer than typical JavaScript deployments:

- **First build:** 3–8 minutes (downloading and compiling all dependencies)
- **Subsequent builds:** 1–4 minutes (cached dependencies)

### Build Caching

Vercel caches the Cargo registry and compiled dependencies between builds. If you experience stale cache issues:

1. Go to **Settings → General** in the Vercel dashboard
2. Scroll to **Build & Development Settings**
3. Click **Override** on the build command and clear the cache, or redeploy with the **Force New Build** option

---

## Troubleshooting

### Cold Starts

Vercel Serverless Functions experience cold starts when the function has not been invoked recently. For Rust functions:

- **Cold start time:** Typically 500ms–2s depending on initialization (database pool creation, migrations check, admin seeding)
- **Warm invocations:** Sub-100ms response times

To minimize cold start impact:

- The database pool is configured with `max_connections(5)` and `acquire_timeout(10s)` to balance resource usage
- Migrations use idempotent checks (`IF NOT EXISTS`) to avoid unnecessary work on warm starts
- Admin seeding performs a single SELECT query to check existence before attempting an INSERT

### Binary Size

Rust serverless functions can produce large binaries. To reduce binary size:

1. Ensure `Cargo.toml` uses release profile optimizations (Vercel handles this automatically)
2. Avoid unnecessary dependencies
3. The current dependency set is optimized for the feature requirements

If you hit Vercel's 50MB function size limit:

```toml
# Add to Cargo.toml for smaller binaries
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

### CORS Issues

CORS is configured in the application via `tower_http::cors::CorsLayer`:

- **Allowed origins:** Any (`*`)
- **Allowed methods:** Any
- **Allowed headers:** `Authorization`, `Content-Type`

If you encounter CORS errors:

1. Verify the API routes are correctly proxied through `/api/*` in `vercel.json`
2. Check that preflight `OPTIONS` requests are reaching the Rust function
3. For production, consider restricting `allow_origin` to your specific domain instead of `Any`

### Database Connection Errors

Common database connection issues and solutions:

| Error | Cause | Solution |
|---|---|---|
| `error connecting to server: tls error` | Missing `sslmode=require` | Add `?sslmode=require` to `DATABASE_URL` |
| `pool timed out while waiting for an open connection` | Connection pool exhausted | Reduce `max_connections` or check for connection leaks |
| `password authentication failed` | Incorrect credentials | Verify `DATABASE_URL` in Vercel environment variables |
| `endpoint is disabled` | Neon endpoint suspended | Wake the endpoint in the Neon dashboard or enable auto-suspend settings |

### JWT Authentication Errors

| Error | Cause | Solution |
|---|---|---|
| `Missing authorization header` | No `Authorization` header sent | Ensure the frontend includes `Bearer <token>` in requests |
| `Token has expired` | JWT token older than 24 hours | Re-authenticate to obtain a new token |
| `Invalid token` | Token signed with different secret | Ensure `JWT_SECRET` matches between environments |

### Migration Failures

If migrations fail on deployment:

1. Check the Vercel function logs for specific SQL errors
2. Connect to the Neon database directly using `psql` or the Neon SQL Editor
3. Verify the database user has `CREATE TABLE` and `CREATE EXTENSION` permissions
4. If tables are in a broken state, drop them manually and redeploy:

```sql
DROP TABLE IF EXISTS posts;
DROP TABLE IF EXISTS users;
DROP EXTENSION IF EXISTS "uuid-ossp";
```

---

## Local Development

### 1. Environment Setup

```bash
cp .env.example .env
# Edit .env with your Neon connection string and a local JWT secret
```

### 2. Run Locally

The application is designed for Vercel's serverless runtime. For local development, you can test individual components:

```bash
# Run tests (requires DATABASE_URL to be set)
cargo test

# Check compilation
cargo check

# Build the binary
cargo build --release
```

### 3. Running Tests

Integration tests require a running PostgreSQL database:

```bash
# Set the database URL
export DATABASE_URL="postgresql://user:pass@localhost/writespace_test?sslmode=require"

# Run all tests
cargo test

# Run a specific test
cargo test test_login_success

# Run tests with output
cargo test -- --nocapture
```

> **Warning:** Tests create and delete data in the database specified by `DATABASE_URL`. Use a dedicated test database, not your production database.

---

## Production Checklist

Before deploying to production, verify:

- [ ] `JWT_SECRET` is set to a strong, unique random string (at least 32 characters)
- [ ] `DEFAULT_ADMIN_PASSWORD` is changed from the default value
- [ ] `DATABASE_URL` points to your production Neon database with `sslmode=require`
- [ ] Neon database region matches or is close to your Vercel deployment region
- [ ] Custom domain is configured with SSL
- [ ] CORS origins are restricted to your domain (optional but recommended)
- [ ] `RUST_LOG` is set to an appropriate level (e.g., `writespace=info`)
- [ ] Neon auto-suspend timeout is configured appropriately for your traffic patterns
- [ ] Database backups are enabled in the Neon dashboard