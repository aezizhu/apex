//! Plugin execution sandbox.
//!
//! Provides a configurable sandbox that constrains plugin execution
//! by enforcing resource limits, permission checks, and isolation boundaries.

use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::manifest::PluginPermission;

// ═══════════════════════════════════════════════════════════════════════════════
// Sandbox Policy
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration that governs what a sandboxed plugin is allowed to do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    /// Permissions granted to the plugin.
    pub granted_permissions: HashSet<PluginPermission>,

    /// Maximum execution wall-clock time.
    #[serde(with = "humantime_serde")]
    pub max_execution_time: Duration,

    /// Maximum memory the plugin may allocate (in bytes). 0 = unlimited.
    pub max_memory_bytes: u64,

    /// Maximum number of outbound network requests. 0 = unlimited.
    pub max_network_requests: u32,

    /// Allowed network hosts (empty = all hosts when Network permission is granted).
    #[serde(default)]
    pub allowed_hosts: Vec<String>,

    /// Filesystem paths the plugin is allowed to read (if FileRead granted).
    #[serde(default)]
    pub allowed_read_paths: Vec<String>,

    /// Filesystem paths the plugin is allowed to write (if FileWrite granted).
    #[serde(default)]
    pub allowed_write_paths: Vec<String>,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            granted_permissions: HashSet::new(),
            max_execution_time: Duration::from_secs(30),
            max_memory_bytes: 256 * 1024 * 1024, // 256 MiB
            max_network_requests: 100,
            allowed_hosts: Vec::new(),
            allowed_read_paths: Vec::new(),
            allowed_write_paths: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Sandbox Context
// ═══════════════════════════════════════════════════════════════════════════════

/// Runtime state tracked while a plugin is executing inside the sandbox.
#[derive(Debug)]
pub struct SandboxContext {
    policy: SandboxPolicy,
    network_requests_made: u32,
    memory_allocated: u64,
}

impl SandboxContext {
    /// Create a new sandbox context with the given policy.
    pub fn new(policy: SandboxPolicy) -> Self {
        Self {
            policy,
            network_requests_made: 0,
            memory_allocated: 0,
        }
    }

    /// Check whether a specific permission is granted.
    pub fn has_permission(&self, perm: &PluginPermission) -> bool {
        self.policy.granted_permissions.contains(perm)
    }

    /// Request to perform a network call. Returns an error if the limit is exceeded
    /// or the Network permission is not granted.
    pub fn request_network(&mut self, host: &str) -> Result<(), SandboxViolation> {
        if !self.has_permission(&PluginPermission::Network) {
            return Err(SandboxViolation::PermissionDenied(PluginPermission::Network));
        }

        if self.policy.max_network_requests > 0
            && self.network_requests_made >= self.policy.max_network_requests
        {
            return Err(SandboxViolation::NetworkRequestLimitExceeded {
                limit: self.policy.max_network_requests,
            });
        }

        if !self.policy.allowed_hosts.is_empty()
            && !self.policy.allowed_hosts.iter().any(|h| h == host)
        {
            return Err(SandboxViolation::HostNotAllowed(host.to_string()));
        }

        self.network_requests_made += 1;
        Ok(())
    }

    /// Request to allocate memory. Returns an error if the limit is exceeded.
    pub fn request_memory(&mut self, bytes: u64) -> Result<(), SandboxViolation> {
        let new_total = self.memory_allocated.saturating_add(bytes);
        if self.policy.max_memory_bytes > 0 && new_total > self.policy.max_memory_bytes {
            return Err(SandboxViolation::MemoryLimitExceeded {
                requested: bytes,
                limit: self.policy.max_memory_bytes,
                current: self.memory_allocated,
            });
        }
        self.memory_allocated = new_total;
        Ok(())
    }

    /// Check whether a file read is permitted.
    pub fn check_file_read(&self, path: &str) -> Result<(), SandboxViolation> {
        if !self.has_permission(&PluginPermission::FileRead) {
            return Err(SandboxViolation::PermissionDenied(PluginPermission::FileRead));
        }

        if !self.policy.allowed_read_paths.is_empty()
            && !self
                .policy
                .allowed_read_paths
                .iter()
                .any(|p| path.starts_with(p))
        {
            return Err(SandboxViolation::PathNotAllowed(path.to_string()));
        }
        Ok(())
    }

    /// Check whether a file write is permitted.
    pub fn check_file_write(&self, path: &str) -> Result<(), SandboxViolation> {
        if !self.has_permission(&PluginPermission::FileWrite) {
            return Err(SandboxViolation::PermissionDenied(PluginPermission::FileWrite));
        }

        if !self.policy.allowed_write_paths.is_empty()
            && !self
                .policy
                .allowed_write_paths
                .iter()
                .any(|p| path.starts_with(p))
        {
            return Err(SandboxViolation::PathNotAllowed(path.to_string()));
        }
        Ok(())
    }

    /// Get the maximum execution duration for this sandbox.
    pub fn max_execution_time(&self) -> Duration {
        self.policy.max_execution_time
    }

    /// Get the underlying policy.
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Sandbox Violations
// ═══════════════════════════════════════════════════════════════════════════════

/// Violations that occur when a plugin exceeds its sandbox constraints.
#[derive(Debug, thiserror::Error)]
pub enum SandboxViolation {
    #[error("Permission denied: {0:?}")]
    PermissionDenied(PluginPermission),

    #[error("Network request limit exceeded (limit: {limit})")]
    NetworkRequestLimitExceeded { limit: u32 },

    #[error("Host not allowed: {0}")]
    HostNotAllowed(String),

    #[error("Memory limit exceeded: requested {requested} bytes, limit {limit}, current {current}")]
    MemoryLimitExceeded {
        requested: u64,
        limit: u64,
        current: u64,
    },

    #[error("Path not allowed: {0}")]
    PathNotAllowed(String),

    #[error("Execution time limit exceeded")]
    ExecutionTimeLimitExceeded,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_with_network() -> SandboxPolicy {
        let mut perms = HashSet::new();
        perms.insert(PluginPermission::Network);
        SandboxPolicy {
            granted_permissions: perms,
            max_network_requests: 2,
            allowed_hosts: vec!["api.example.com".into()],
            ..Default::default()
        }
    }

    #[test]
    fn test_network_permission_required() {
        let mut ctx = SandboxContext::new(SandboxPolicy::default());
        assert!(ctx.request_network("localhost").is_err());
    }

    #[test]
    fn test_network_allowed_host() {
        let mut ctx = SandboxContext::new(policy_with_network());
        assert!(ctx.request_network("api.example.com").is_ok());
        assert!(ctx.request_network("evil.com").is_err());
    }

    #[test]
    fn test_network_request_limit() {
        let mut ctx = SandboxContext::new(policy_with_network());
        assert!(ctx.request_network("api.example.com").is_ok());
        assert!(ctx.request_network("api.example.com").is_ok());
        assert!(ctx.request_network("api.example.com").is_err()); // third request
    }

    #[test]
    fn test_memory_limit() {
        let policy = SandboxPolicy {
            max_memory_bytes: 1024,
            ..Default::default()
        };
        let mut ctx = SandboxContext::new(policy);
        assert!(ctx.request_memory(512).is_ok());
        assert!(ctx.request_memory(512).is_ok());
        assert!(ctx.request_memory(1).is_err());
    }

    #[test]
    fn test_file_read_permission() {
        let mut perms = HashSet::new();
        perms.insert(PluginPermission::FileRead);
        let policy = SandboxPolicy {
            granted_permissions: perms,
            allowed_read_paths: vec!["/data/".into()],
            ..Default::default()
        };
        let ctx = SandboxContext::new(policy);
        assert!(ctx.check_file_read("/data/file.txt").is_ok());
        assert!(ctx.check_file_read("/etc/passwd").is_err());
    }

    #[test]
    fn test_file_write_without_permission() {
        let ctx = SandboxContext::new(SandboxPolicy::default());
        assert!(ctx.check_file_write("/tmp/out.txt").is_err());
    }
}
