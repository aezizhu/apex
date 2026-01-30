# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security seriously at Project Apex. If you discover a security vulnerability, please follow these steps:

### Do NOT

- Open a public GitHub issue
- Disclose the vulnerability publicly before it's fixed
- Exploit the vulnerability beyond what's necessary to demonstrate it

### Do

1. **Email us directly** at security@apex-swarm.io (or create a private security advisory on GitHub)
2. **Include details**:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Any suggested fixes (optional)

### What to expect

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 1 week
- **Resolution timeline**: Depends on severity
  - Critical: 24-72 hours
  - High: 1-2 weeks
  - Medium: 2-4 weeks
  - Low: Next release cycle

### Severity Levels

| Level | Description | Examples |
|-------|-------------|----------|
| **Critical** | Immediate threat to production systems | RCE, authentication bypass, data exfiltration |
| **High** | Significant security impact | Privilege escalation, SQL injection, XSS |
| **Medium** | Limited security impact | Information disclosure, CSRF |
| **Low** | Minimal security impact | Minor information leaks, best practice violations |

## Security Best Practices

When deploying Apex, follow these guidelines:

### API Keys & Secrets

```bash
# Never commit secrets to version control
# Use environment variables or secret management
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."

# Or use Kubernetes secrets
kubectl create secret generic apex-secrets \
  --from-literal=openai-api-key="sk-..." \
  --from-literal=anthropic-api-key="sk-ant-..."
```

### Network Security

- Run the API behind a reverse proxy (nginx, Traefik)
- Enable TLS/HTTPS in production
- Use network policies in Kubernetes
- Restrict database access to internal networks only

### Authentication

- Enable API key authentication for all endpoints
- Use short-lived JWT tokens
- Implement rate limiting
- Log all authentication attempts

### Agent Contracts

- Always set resource limits (tokens, cost, time)
- Use the approval queue for high-cost actions
- Monitor contract violations in real-time
- Set up alerts for unusual spending patterns

### Database Security

- Use strong passwords for PostgreSQL
- Enable SSL connections
- Regular backups with encryption
- Principle of least privilege for database users

### Container Security

- Use non-root users in containers
- Scan images for vulnerabilities
- Keep base images updated
- Use read-only filesystems where possible

## Security Features

Apex includes several built-in security features:

### Agent Contracts

```rust
// Enforce resource limits
let limits = ResourceLimits {
    token_limit: 10000,
    cost_limit: 1.0,
    api_call_limit: 100,
    time_limit_seconds: 300,
};
```

### Approval Queue

High-impact actions can require human approval:

```python
# Actions over $10 require approval
if estimated_cost > 10.0:
    await approval_queue.request_approval(action)
```

### Audit Logging

All actions are logged with:
- Timestamp
- Agent ID
- Action type
- Resource usage
- Trace ID for distributed tracing

### Circuit Breaker

Automatic protection against cascading failures:

```rust
// Circuit breaker trips after 5 failures
let circuit_breaker = CircuitBreaker::new(
    failure_threshold: 5,
    recovery_timeout: Duration::from_secs(30),
);
```

## Compliance

Apex supports compliance with:

- **SOC 2**: Audit logging, access controls
- **GDPR**: Data retention policies, right to deletion
- **HIPAA**: When properly configured with encryption

## Contact

- Security issues: security@apex-swarm.io
- General questions: hello@apex-swarm.io
- GitHub Security Advisories: [Create Advisory](https://github.com/apex-swarm/apex/security/advisories/new)
