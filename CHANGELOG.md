# Changelog

All notable changes to the WriteSpace project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2024-12-01

### Added

#### Authentication
- JWT-based authentication with 24-hour token expiration
- User login with username and password via `POST /api/auth/login`
- User self-registration via `POST /api/auth/register`
- Secure password hashing using bcrypt with cost factor 12
- Default admin account seeded on first startup from environment variables
- Bearer token authorization header support across all protected endpoints

#### Blog CRUD
- Create blog posts via `POST /api/posts` (authenticated users)
- List all posts via `GET /api/posts` (authenticated users see all; public visitors see latest 3)
- Get single post via `GET /api/posts/{id}` (public)
- Update posts via `PUT /api/posts/{id}` (owner or admin)
- Delete posts via `DELETE /api/posts/{id}` (owner or admin)
- Post responses include `can_edit` and `can_delete` permission flags per viewer

#### Role-Based Access Control
- Two roles: `admin` and `user`
- Admins can edit and delete any post
- Regular users can only edit and delete their own posts
- Admin-only endpoints protected with role checks
- Default admin account marked with `is_default_admin` flag and cannot be deleted

#### Admin Dashboard
- Platform statistics endpoint via `GET /api/admin/stats` (total posts, users, admins, recent posts)
- User listing via `GET /api/admin/users` (admin only)
- User creation via `POST /api/admin/users` with role assignment (admin only)
- User deletion via `DELETE /api/admin/users/{id}` with safeguards (admin only)
- Protection against deleting the default admin account or self-deletion

#### Static Frontend
- Landing page with feature highlights and latest posts preview
- Login and registration pages with client-side validation
- Blog post listing with card grid layout and excerpt previews
- Single post view with author info and action buttons
- Post editor supporting both create and edit modes with character counter
- Admin dashboard with statistics cards and recent posts list
- User management page with create and delete modals
- Responsive navigation bar with role-aware links and avatar badges
- Tailwind CSS via CDN for all styling
- Shared `app.js` utility library for API requests, token management, JWT decoding, date formatting, navigation, and error display

#### Infrastructure
- Rust backend built with Axum 0.8 web framework
- Vercel serverless deployment via `vercel_runtime` and `@vercel/rust` builder
- Neon serverless PostgreSQL database with `sslmode=require`
- SQLx async PostgreSQL driver with connection pooling
- Automatic database migration on startup via embedded SQL
- CORS configuration allowing all origins for API access
- Structured logging with `tracing` and `tower_http::trace::TraceLayer`
- Environment-based configuration via `.env` file

#### Developer Experience
- Comprehensive integration test suite covering auth, posts, admin, and permissions
- `.env.example` with documented configuration variables
- `vercel.json` with routing rules for API and static assets