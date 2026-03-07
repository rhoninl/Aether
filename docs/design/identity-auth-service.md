# Identity & Auth Service Design Document

**Task**: task-011
**Date**: 2026-03-07
**Status**: Design Phase
**Assignee**: @claude-005

---

## Background

The Aether platform requires a centralized identity and authentication service that handles player authentication, session management, profile management, and authorization. As per the DESIGN.md (Section 4.1, 6.1), identity is centralized — self-hosted world servers verify player tokens against this central auth service.

## Why

- All backend services and world servers need a trusted identity provider
- Players need secure authentication with modern methods (OAuth2, WebAuthn)
- Self-hosted (federated) worlds need a token validation endpoint
- Authorization (RBAC/ABAC) must be enforced consistently across the platform
- Audit logging is required for security and compliance

## What

Implement a Go-based Identity & Auth microservice with:
1. JWT-based authentication with Ed25519 signing
2. OAuth2 social login (Google, Apple, Discord, Steam)
3. Session management with refresh tokens
4. Player profile CRUD
5. Token validation endpoint for world servers
6. RBAC/ABAC permission model
7. WebAuthn/passkey support
8. Audit logging

## How

### Technology Stack

| Component | Technology | Rationale |
|---|---|---|
| Language | Go | Per DESIGN.md Section 8.2 |
| HTTP Framework | net/http + chi router | Lightweight, stdlib-compatible |
| Database | PostgreSQL 16 | ACID, JSONB for settings |
| Cache | Redis 7 | Session storage, token blacklist |
| Message Bus | NATS JetStream | Auth event publishing |
| JWT | go-jose/v4 | Ed25519/RSA signing |
| WebAuthn | go-webauthn | FIDO2/passkey support |
| Migration | golang-migrate | DB schema versioning |

### Project Structure

```
services/identity/
  cmd/
    server/
      main.go              # Entry point
  internal/
    config/
      config.go            # Environment-based configuration
    handler/
      auth.go              # Login, register, refresh, logout
      profile.go           # Profile CRUD
      oauth.go             # OAuth2 callbacks
      webauthn.go          # WebAuthn registration/auth
      token.go             # Token validation (for world servers)
      middleware.go         # Auth middleware, rate limiting
    model/
      user.go              # User, Session, Role models
      audit.go             # Audit log model
    repository/
      user.go              # User DB operations
      session.go           # Session DB operations
      audit.go             # Audit log DB operations
    service/
      auth.go              # Auth business logic
      profile.go           # Profile business logic
      oauth.go             # OAuth2 flow logic
      webauthn.go          # WebAuthn logic
      permission.go        # RBAC/ABAC logic
    migration/
      000001_init.up.sql
      000001_init.down.sql
  go.mod
  go.sum
  Dockerfile
```

### Database Design

```mermaid
erDiagram
    users {
        uuid id PK
        string email UK
        string username UK
        string password_hash "nullable (OAuth/WebAuthn users)"
        string display_name
        string bio
        string avatar_url
        jsonb settings
        string role "player|creator|moderator|admin"
        boolean email_verified
        timestamptz created_at
        timestamptz updated_at
        timestamptz deleted_at "soft delete"
    }

    sessions {
        uuid id PK
        uuid user_id FK
        string refresh_token_hash UK
        string ip_address
        string user_agent
        timestamptz expires_at
        timestamptz created_at
    }

    oauth_accounts {
        uuid id PK
        uuid user_id FK
        string provider "google|apple|discord|steam"
        string provider_user_id
        string access_token_enc
        string refresh_token_enc
        timestamptz created_at
    }

    webauthn_credentials {
        uuid id PK
        uuid user_id FK
        bytea credential_id UK
        bytea public_key
        string aaguid
        int sign_count
        timestamptz created_at
    }

    roles {
        uuid id PK
        string name UK "player|creator|moderator|admin"
        jsonb permissions "ABAC permission set"
        int hierarchy_level "0=player, 10=creator, 50=mod, 100=admin"
        timestamptz created_at
    }

    user_roles {
        uuid id PK
        uuid user_id FK
        uuid role_id FK
        uuid granted_by FK "admin who granted"
        timestamptz granted_at
        timestamptz revoked_at "nullable"
    }

    audit_logs {
        uuid id PK
        uuid user_id FK "nullable"
        string event_type "login|logout|token_refresh|permission_change|profile_update"
        string ip_address
        string user_agent
        jsonb metadata
        timestamptz created_at
    }

    users ||--o{ sessions : has
    users ||--o{ oauth_accounts : has
    users ||--o{ webauthn_credentials : has
    users ||--o{ user_roles : has
    roles ||--o{ user_roles : defines
    users ||--o{ audit_logs : generates
```

### API Design

#### Authentication

| Method | Endpoint | Description |
|---|---|---|
| POST | /api/v1/auth/register | Register with email/password |
| POST | /api/v1/auth/login | Login with email/password |
| POST | /api/v1/auth/refresh | Refresh access token |
| POST | /api/v1/auth/logout | Revoke session |
| GET | /api/v1/auth/oauth/{provider} | Initiate OAuth2 flow |
| GET | /api/v1/auth/oauth/{provider}/callback | OAuth2 callback |
| POST | /api/v1/auth/webauthn/register/begin | Begin WebAuthn registration |
| POST | /api/v1/auth/webauthn/register/finish | Complete WebAuthn registration |
| POST | /api/v1/auth/webauthn/login/begin | Begin WebAuthn login |
| POST | /api/v1/auth/webauthn/login/finish | Complete WebAuthn login |

#### Token Validation (for world servers)

| Method | Endpoint | Description |
|---|---|---|
| POST | /api/v1/auth/validate | Validate JWT, return claims |
| GET | /api/v1/auth/.well-known/jwks.json | Public JWKS endpoint |

#### Profile

| Method | Endpoint | Description |
|---|---|---|
| GET | /api/v1/profiles/me | Get current user profile |
| PUT | /api/v1/profiles/me | Update current user profile |
| GET | /api/v1/profiles/{id} | Get user profile by ID |
| GET | /api/v1/profiles | Search profiles |

#### Permissions

| Method | Endpoint | Description |
|---|---|---|
| GET | /api/v1/permissions/me | Get own permissions |
| POST | /api/v1/admin/roles/{user_id} | Assign role (admin only) |
| DELETE | /api/v1/admin/roles/{user_id}/{role} | Revoke role (admin only) |

### JWT Token Design

**Access Token** (short-lived, 15 minutes):
```json
{
  "sub": "user-uuid",
  "iss": "aether-identity",
  "aud": ["aether-api", "aether-world"],
  "exp": 1709890800,
  "iat": 1709889900,
  "jti": "unique-token-id",
  "role": "creator",
  "permissions": ["world:create", "avatar:upload"]
}
```

**Refresh Token** (long-lived, 30 days):
- Stored as SHA-256 hash in `sessions` table
- Rotated on each refresh (old token invalidated)
- Bound to IP/user-agent for anomaly detection

**Signing**: Ed25519 (fast, small signatures, quantum-resistant compared to RSA)

### RBAC/ABAC Permission Model

```mermaid
graph TD
    Admin["Admin (level 100)"]
    Moderator["Moderator (level 50)"]
    Creator["Creator (level 10)"]
    Player["Player (level 0)"]

    Admin -->|inherits| Moderator
    Moderator -->|inherits| Creator
    Creator -->|inherits| Player

    Player --- P1["profile:read, profile:write, world:join, avatar:equip"]
    Creator --- P2["world:create, world:edit, avatar:upload, asset:upload"]
    Moderator --- P3["user:warn, user:mute, user:kick, report:review"]
    Admin --- P4["user:ban, role:assign, role:revoke, system:configure"]
```

Roles are hierarchical: higher-level roles inherit all permissions from lower levels.

### Auth Flow

```mermaid
sequenceDiagram
    participant C as Client
    participant G as API Gateway
    participant A as Auth Service
    participant DB as PostgreSQL
    participant R as Redis
    participant N as NATS

    C->>G: POST /auth/login {email, password}
    G->>A: Forward request
    A->>DB: Query user by email
    DB-->>A: User record
    A->>A: Verify password (argon2id)
    A->>DB: Create session
    A->>R: Cache session
    A->>N: Publish auth.login event
    A->>DB: Write audit log
    A-->>G: {access_token, refresh_token}
    G-->>C: {access_token, refresh_token}

    Note over C,A: Later: Token refresh
    C->>G: POST /auth/refresh {refresh_token}
    G->>A: Forward
    A->>R: Check session (cache)
    alt Cache miss
        A->>DB: Lookup session
    end
    A->>A: Validate refresh token
    A->>A: Generate new token pair
    A->>DB: Rotate refresh token
    A->>R: Update cache
    A-->>G: {new_access_token, new_refresh_token}
    G-->>C: {new_access_token, new_refresh_token}
```

### World Server Token Validation Flow

```mermaid
sequenceDiagram
    participant WS as World Server
    participant A as Auth Service

    Note over WS,A: Option 1: JWKS (preferred, no network call per request)
    WS->>A: GET /.well-known/jwks.json (cached, refresh every 5 min)
    A-->>WS: JWKS with Ed25519 public keys
    WS->>WS: Validate JWT locally using cached JWKS

    Note over WS,A: Option 2: Validation endpoint (for extra claims)
    WS->>A: POST /auth/validate {token}
    A-->>WS: {valid: true, claims: {...}}
```

### Test Design

| Category | Tests |
|---|---|
| Unit: Auth | Password hashing, JWT creation/validation, token rotation |
| Unit: RBAC | Permission inheritance, role hierarchy, ABAC evaluation |
| Unit: Profile | CRUD validation, sanitization |
| Integration: Auth | Full login/register/refresh/logout flow |
| Integration: OAuth | OAuth2 flow with mocked providers |
| Integration: WebAuthn | Passkey registration/authentication flow |
| Integration: Validation | World server token validation via JWKS |
| Integration: Audit | Event logging for all auth operations |
| Repository | User CRUD, session management, audit log writes |

### Configuration (Environment Variables)

| Variable | Description | Default |
|---|---|---|
| `IDENTITY_PORT` | HTTP listen port | `8080` |
| `IDENTITY_DB_URL` | PostgreSQL connection string | required |
| `IDENTITY_REDIS_URL` | Redis connection string | required |
| `IDENTITY_NATS_URL` | NATS connection string | required |
| `IDENTITY_JWT_PRIVATE_KEY` | Ed25519 private key (PEM) | required |
| `IDENTITY_JWT_ACCESS_TTL` | Access token TTL | `15m` |
| `IDENTITY_JWT_REFRESH_TTL` | Refresh token TTL | `720h` |
| `IDENTITY_OAUTH_GOOGLE_ID` | Google OAuth client ID | optional |
| `IDENTITY_OAUTH_GOOGLE_SECRET` | Google OAuth client secret | optional |
| `IDENTITY_OAUTH_APPLE_ID` | Apple OAuth client ID | optional |
| `IDENTITY_OAUTH_DISCORD_ID` | Discord OAuth client ID | optional |
| `IDENTITY_OAUTH_STEAM_KEY` | Steam API key | optional |
| `IDENTITY_ARGON2_MEMORY` | Argon2id memory (KB) | `65536` |
| `IDENTITY_ARGON2_ITERATIONS` | Argon2id iterations | `3` |
| `IDENTITY_ARGON2_PARALLELISM` | Argon2id parallelism | `2` |
| `IDENTITY_RATE_LIMIT_LOGIN` | Login rate limit (per min) | `10` |
| `IDENTITY_RATE_LIMIT_REGISTER` | Register rate limit (per min) | `5` |
