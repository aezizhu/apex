## Summary

This issue tracks multiple critical security vulnerabilities found in the Apex codebase that require immediate attention.

## Issues Found

### 1. Hardcoded Database Credentials (Critical)
**File:** `src/backend/core/src/main.rs:32`
- Default fallback config contains hardcoded credentials: `postgres://apex:apex_secret@localhost:5432/apex`
- Creates security vulnerability if config fails to load

### 2. Default JWT Secret (Critical)
**File:** `src/backend/core/src/websocket/mod.rs:78`
- Default JWT secret is `change-me-in-production`
- Will be used if no secret is configured

### 3. Unrestricted CORS (Critical)
**File:** `src/backend/core/src/api/mod.rs:112-115`
- `CorsLayer` configured with `allow_origin(Any)` 
- Permits cross-origin requests from any domain

### 4. Authentication Enabled by Default
**File:** `src/backend/core/src/middleware/auth.rs:398`
- `AuthConfig::default()` sets `enabled: true` but `jwt_secret` is None
- Will cause runtime errors

### 5. Default JWT Secret in WebSocket
**File:** `src/backend/core/src/websocket/mod.rs:63`
- WebSocket JWT secret stored in config which could be logged

### 6. Task Creation Without Authentication
**File:** `src/backend/core/src/api/handlers.rs:162`
- `create_task` handler doesn't check or associate with authenticated user
- Any client can create tasks without authentication

## Required Fixes

1. **main.rs**: Remove hardcoded credentials, fail fast if config is missing required fields
2. **websocket/mod.rs**: Require JWT secret to be explicitly configured, panic if not provided in production
3. **api/mod.rs**: Configure CORS to use specific allowed origins from configuration
4. **middleware/auth.rs**: Set `enabled: false` by default, require explicit configuration
5. **api/handlers.rs**: Add authentication check and associate tasks with user/org ID from AuthContext

## Priority
**CRITICAL** - These issues expose the application to security vulnerabilities and must be fixed before any production deployment.
