# Project Apex Security Design Document

**Version:** 1.0
**Last Updated:** 2026-01-29
**Status:** Draft
**Classification:** Internal

---

## Table of Contents

1. [Overview](#1-overview)
2. [Resource Limit Enforcement](#2-resource-limit-enforcement)
3. [Tool Sandboxing](#3-tool-sandboxing)
4. [Secret Management](#4-secret-management)
5. [Audit Trail](#5-audit-trail)
6. [Authentication & Authorization](#6-authentication--authorization)
7. [Compliance](#7-compliance)
8. [Incident Response](#8-incident-response)
9. [Security Checklist](#9-security-checklist)

---

## 1. Overview

### 1.1 Purpose

This document defines the security architecture for Project Apex, an autonomous multi-agent system. Given the autonomous nature of agents executing tasks with minimal human oversight, security controls must be comprehensive, defense-in-depth, and fail-safe.

### 1.2 Security Principles

- **Least Privilege:** Agents and users receive only the permissions required for their tasks
- **Defense in Depth:** Multiple security layers protect against single points of failure
- **Fail-Safe Defaults:** System fails closed; undefined behavior is denied
- **Complete Mediation:** Every access request is checked against authorization policies
- **Separation of Duties:** Critical operations require multiple approvals
- **Audit Everything:** All actions are logged for forensic analysis

### 1.3 Threat Model

| Threat Actor | Capabilities | Mitigations |
|--------------|--------------|-------------|
| Malicious Agent | Prompt injection, resource abuse | Sandboxing, resource limits |
| Compromised Tool | Data exfiltration, lateral movement | Network isolation, output validation |
| Insider Threat | Credential theft, unauthorized access | MFA, RBAC, audit logging |
| External Attacker | API abuse, injection attacks | Rate limiting, input validation |

---

## 2. Resource Limit Enforcement

### 2.1 Hard Limits

Resource limits prevent runaway costs and ensure fair resource allocation across agents.

#### 2.1.1 Token Budget Enforcement

```yaml
token_limits:
  per_task:
    input_tokens: 100000      # Max input tokens per task
    output_tokens: 50000      # Max output tokens per task
    total_tokens: 150000      # Combined limit
  per_agent:
    hourly: 500000            # Tokens per hour per agent
    daily: 5000000            # Tokens per day per agent
  global:
    hourly: 10000000          # System-wide hourly limit
    daily: 100000000          # System-wide daily limit
```

**Enforcement Algorithm:**

```python
class TokenBudgetEnforcer:
    def check_budget(self, request: TokenRequest) -> BudgetDecision:
        # Check in order: task -> agent -> global
        checks = [
            self._check_task_budget(request),
            self._check_agent_budget(request),
            self._check_global_budget(request),
        ]

        for check in checks:
            if not check.allowed:
                return BudgetDecision(
                    allowed=False,
                    reason=check.reason,
                    remaining=check.remaining
                )

        # Reserve tokens before execution
        self._reserve_tokens(request)
        return BudgetDecision(allowed=True)

    def finalize_usage(self, actual_tokens: int):
        # Release unused reserved tokens
        # Update running totals
        pass
```

#### 2.1.2 Cost Budget Enforcement

```yaml
cost_limits:
  per_task:
    max_cost_usd: 1.00        # Default per-task limit
    warning_threshold: 0.80   # Warn at 80% consumption
  per_agent:
    hourly_usd: 10.00
    daily_usd: 100.00
    monthly_usd: 1000.00
  per_user:
    monthly_usd: 5000.00      # User-level billing limit
  global:
    daily_usd: 10000.00
    monthly_usd: 100000.00
```

**Real-Time Cost Tracking:**

```python
class CostTracker:
    def __init__(self):
        self.pricing = {
            "gpt-4": {"input": 0.03, "output": 0.06},  # per 1K tokens
            "claude-3": {"input": 0.015, "output": 0.075},
            "tool_execution": 0.001,  # per execution
        }

    def calculate_cost(self, usage: Usage) -> Decimal:
        model_cost = (
            usage.input_tokens * self.pricing[usage.model]["input"] / 1000 +
            usage.output_tokens * self.pricing[usage.model]["output"] / 1000
        )
        tool_cost = usage.tool_calls * self.pricing["tool_execution"]
        return Decimal(model_cost + tool_cost).quantize(Decimal("0.0001"))

    def check_and_update(self, entity_id: str, cost: Decimal) -> bool:
        with self.lock:
            current = self.get_current_spend(entity_id)
            limit = self.get_limit(entity_id)

            if current + cost > limit:
                self.emit_alert(entity_id, "BUDGET_EXCEEDED")
                return False

            self.update_spend(entity_id, cost)

            if (current + cost) / limit > 0.8:
                self.emit_warning(entity_id, "BUDGET_WARNING")

            return True
```

#### 2.1.3 API Call Rate Limiting

```yaml
rate_limits:
  per_tool:
    web_search:
      requests_per_minute: 10
      requests_per_hour: 100
      burst_size: 5
    code_execution:
      requests_per_minute: 20
      requests_per_hour: 200
      burst_size: 10
    database_query:
      requests_per_minute: 30
      requests_per_hour: 500
      burst_size: 15
  per_agent:
    requests_per_minute: 60
    requests_per_hour: 1000
  global:
    requests_per_second: 100
    requests_per_minute: 3000
```

**Token Bucket Implementation:**

```python
class TokenBucketRateLimiter:
    def __init__(self, rate: float, capacity: int):
        self.rate = rate          # Tokens per second
        self.capacity = capacity   # Max burst size
        self.tokens = capacity
        self.last_update = time.monotonic()
        self.lock = threading.Lock()

    def acquire(self, tokens: int = 1) -> tuple[bool, float]:
        with self.lock:
            now = time.monotonic()
            elapsed = now - self.last_update

            # Refill tokens
            self.tokens = min(
                self.capacity,
                self.tokens + elapsed * self.rate
            )
            self.last_update = now

            if self.tokens >= tokens:
                self.tokens -= tokens
                return True, 0.0
            else:
                wait_time = (tokens - self.tokens) / self.rate
                return False, wait_time
```

#### 2.1.4 Wall-Clock Time Limits

```yaml
time_limits:
  task:
    default_timeout: 300        # 5 minutes
    max_timeout: 3600           # 1 hour hard limit
    warning_at: 0.8             # Warn at 80% elapsed
  tool_execution:
    code_execution: 30          # 30 seconds
    web_search: 10              # 10 seconds
    database_query: 5           # 5 seconds
    file_operation: 5           # 5 seconds
  agent_session:
    max_duration: 86400         # 24 hours
    idle_timeout: 3600          # 1 hour
```

#### 2.1.5 Memory Limits

```yaml
memory_limits:
  agent_process:
    max_heap_mb: 512
    max_stack_mb: 8
    soft_limit_mb: 384          # Trigger GC
  tool_execution:
    code_sandbox_mb: 512
    result_buffer_mb: 10
  global:
    max_concurrent_agents: 100
    reserved_system_mb: 2048
```

### 2.2 Contract Conservation Law

Parent contracts must have sufficient budget to cover all child operations.

#### 2.2.1 Budget Inheritance Model

```
Parent Contract Budget >= Sum(Child Budgets) + Overhead + Reserve

Where:
  - Overhead = 10% of allocated child budgets (coordination cost)
  - Reserve = 5% of parent budget (buffer for retries)
```

#### 2.2.2 Enforcement Algorithm

```python
class ContractBudgetEnforcer:
    def validate_contract_creation(
        self,
        parent: Contract,
        child_request: ContractRequest
    ) -> ValidationResult:

        # Calculate existing allocations
        existing_children = self.get_child_contracts(parent.id)
        existing_allocation = sum(c.budget for c in existing_children)

        # Calculate overhead
        new_total = existing_allocation + child_request.budget
        overhead = new_total * Decimal("0.10")
        reserve = parent.budget * Decimal("0.05")

        # Check conservation law
        required = new_total + overhead + reserve
        available = parent.budget - parent.spent

        if required > available:
            return ValidationResult(
                valid=False,
                error="BUDGET_CONSERVATION_VIOLATION",
                details={
                    "required": required,
                    "available": available,
                    "shortfall": required - available
                }
            )

        return ValidationResult(valid=True)

    def handle_violation(
        self,
        violation: ConservationViolation,
        policy: ViolationPolicy
    ) -> ViolationResponse:

        if policy == ViolationPolicy.REJECT:
            return ViolationResponse(action="REJECT")

        elif policy == ViolationPolicy.WARN:
            self.emit_warning(violation)
            return ViolationResponse(action="ALLOW_WITH_WARNING")

        elif policy == ViolationPolicy.AUTO_ADJUST:
            adjusted_budget = self.calculate_max_allowable(violation)
            return ViolationResponse(
                action="ADJUST",
                adjusted_budget=adjusted_budget
            )
```

#### 2.2.3 Violation Handling Matrix

| Violation Type | Default Action | Configurable |
|----------------|----------------|--------------|
| Budget Overflow | REJECT | Yes |
| Time Overflow | REJECT | Yes |
| Overhead Exceeded | WARN | Yes |
| Reserve Depleted | WARN + THROTTLE | Yes |

### 2.3 Circuit Breaker

Prevents cascading failures and resource exhaustion from problematic agents.

#### 2.3.1 State Machine

```
                    ┌─────────────────────┐
                    │                     │
         success    │      CLOSED         │    failure
        ┌──────────►│   (normal ops)      │◄──────────┐
        │           │                     │           │
        │           └──────────┬──────────┘           │
        │                      │                      │
        │                      │ failure_threshold    │
        │                      │ reached              │
        │                      ▼                      │
        │           ┌─────────────────────┐           │
        │           │                     │           │
        │           │       OPEN          │───────────┤
        │           │   (rejecting)       │  failure  │
        │           │                     │           │
        │           └──────────┬──────────┘           │
        │                      │                      │
        │                      │ timeout expires      │
        │                      ▼                      │
        │           ┌─────────────────────┐           │
        │           │                     │           │
        └───────────│     HALF_OPEN       │───────────┘
          success   │   (testing)         │
                    │                     │
                    └─────────────────────┘
```

#### 2.3.2 Implementation

```python
class CircuitBreaker:
    def __init__(self, config: CircuitBreakerConfig):
        self.state = CircuitState.CLOSED
        self.failure_count = 0
        self.last_failure_time = None
        self.config = config

        # Loop detection
        self.recent_outputs = deque(maxlen=10)
        self.embedding_model = load_embedding_model()

    def can_execute(self) -> bool:
        if self.state == CircuitState.CLOSED:
            return True

        if self.state == CircuitState.OPEN:
            if self._timeout_expired():
                self.state = CircuitState.HALF_OPEN
                return True
            return False

        if self.state == CircuitState.HALF_OPEN:
            return True  # Allow test request

        return False

    def record_result(self, success: bool, output: str = None):
        if success:
            if self.state == CircuitState.HALF_OPEN:
                self.state = CircuitState.CLOSED
            self.failure_count = 0

            # Check for loops even on success
            if output and self._detect_loop(output):
                self._trip_breaker("LOOP_DETECTED")
        else:
            self.failure_count += 1
            self.last_failure_time = time.time()

            if self._should_trip():
                self._trip_breaker("FAILURE_THRESHOLD")

    def _detect_loop(self, output: str) -> bool:
        """Detect loops via vector similarity."""
        embedding = self.embedding_model.encode(output)

        for prev_embedding in self.recent_outputs:
            similarity = cosine_similarity(embedding, prev_embedding)
            if similarity >= 0.98:  # Threshold for loop detection
                return True

        self.recent_outputs.append(embedding)
        return False

    def _should_trip(self) -> bool:
        """3 failures in 5 minutes."""
        if self.failure_count < 3:
            return False

        window_start = time.time() - 300  # 5 minutes
        recent_failures = sum(
            1 for t in self.failure_times
            if t > window_start
        )
        return recent_failures >= 3
```

#### 2.3.3 Recovery Logic

```python
class CircuitRecovery:
    def attempt_recovery(self, breaker: CircuitBreaker) -> RecoveryResult:
        # Exponential backoff for recovery attempts
        attempt = breaker.recovery_attempts
        wait_time = min(300, 30 * (2 ** attempt))  # Max 5 min

        if time.time() - breaker.last_failure_time < wait_time:
            return RecoveryResult(recovered=False, wait=wait_time)

        # Test with minimal operation
        test_result = self._run_health_check(breaker.agent_id)

        if test_result.healthy:
            breaker.state = CircuitState.CLOSED
            breaker.recovery_attempts = 0
            return RecoveryResult(recovered=True)

        breaker.recovery_attempts += 1
        return RecoveryResult(recovered=False, wait=wait_time * 2)
```

### 2.4 Cost-per-Insight Metric

Measures agent efficiency by tracking meaningful state changes relative to resource consumption.

#### 2.4.1 Metric Definition

```python
@dataclass
class InsightMetrics:
    tokens_consumed: int
    cost_usd: Decimal
    state_mutations: int        # Meaningful changes
    artifacts_produced: int     # Files, reports, etc.
    decisions_made: int         # Branch points taken
    tools_executed: int

    @property
    def cost_per_insight(self) -> Decimal:
        insights = (
            self.state_mutations * 1.0 +
            self.artifacts_produced * 2.0 +
            self.decisions_made * 0.5
        )
        if insights == 0:
            return Decimal("inf")
        return self.cost_usd / Decimal(insights)

    @property
    def efficiency_score(self) -> float:
        # 0-100 scale, higher is better
        baseline_cpi = Decimal("0.10")  # Expected cost per insight
        ratio = float(baseline_cpi / self.cost_per_insight)
        return min(100, max(0, ratio * 50))
```

#### 2.4.2 Alerting

```yaml
efficiency_alerts:
  warning_threshold: 30         # Score below 30 triggers warning
  critical_threshold: 10        # Score below 10 triggers circuit breaker
  evaluation_window: 300        # 5-minute rolling window
  min_samples: 5                # Minimum actions before evaluation
```

---

## 3. Tool Sandboxing

### 3.1 Code Execution Sandbox

#### 3.1.1 Docker Container Configuration

```yaml
# docker-compose.sandbox.yml
version: "3.8"

services:
  code-sandbox:
    image: apex/sandbox:latest
    deploy:
      resources:
        limits:
          cpus: "1.0"
          memory: 512M
        reservations:
          cpus: "0.25"
          memory: 128M
    security_opt:
      - no-new-privileges:true
      - seccomp:sandbox-seccomp.json
      - apparmor:sandbox-apparmor
    cap_drop:
      - ALL
    cap_add: []  # No capabilities
    read_only: true
    tmpfs:
      - /tmp:size=64M,mode=1777
      - /home/sandbox:size=32M,mode=0755
    environment:
      - SANDBOX_TIMEOUT=30
      - SANDBOX_MAX_OUTPUT=1048576
    networks:
      - sandbox-isolated
    pids_limit: 100
    ulimits:
      nproc: 50
      nofile:
        soft: 100
        hard: 200

networks:
  sandbox-isolated:
    driver: bridge
    internal: true  # No external access
```

#### 3.1.2 Seccomp Profile

```json
{
  "defaultAction": "SCMP_ACT_ERRNO",
  "architectures": ["SCMP_ARCH_X86_64"],
  "syscalls": [
    {
      "names": [
        "read", "write", "close", "fstat", "mmap", "mprotect",
        "munmap", "brk", "rt_sigaction", "rt_sigprocmask",
        "access", "getpid", "clone", "execve", "wait4",
        "exit_group", "arch_prctl", "gettid", "futex",
        "set_tid_address", "set_robust_list", "prlimit64",
        "getrandom", "clock_gettime"
      ],
      "action": "SCMP_ACT_ALLOW"
    },
    {
      "names": ["socket", "connect", "bind", "listen", "accept"],
      "action": "SCMP_ACT_ERRNO",
      "errnoRet": 1
    }
  ]
}
```

#### 3.1.3 Execution Flow

```python
class CodeSandbox:
    def execute(self, code: str, language: str, timeout: int = 30) -> ExecutionResult:
        # Validate input
        if len(code) > 100_000:
            raise ValidationError("Code too large")

        # Create ephemeral container
        container_id = self._create_container()

        try:
            # Write code to tmpfs
            self._write_code(container_id, code, language)

            # Execute with timeout
            result = self._run_with_timeout(
                container_id,
                command=self._get_interpreter(language),
                timeout=timeout
            )

            # Capture output (truncated)
            stdout = result.stdout[:1_048_576]  # 1MB limit
            stderr = result.stderr[:102_400]    # 100KB limit

            return ExecutionResult(
                success=result.exit_code == 0,
                stdout=stdout,
                stderr=stderr,
                exit_code=result.exit_code,
                execution_time=result.duration
            )

        finally:
            # Always destroy container
            self._destroy_container(container_id)
```

### 3.2 Web Search Sandbox

#### 3.2.1 Configuration

```yaml
web_search:
  allowed_domains:
    - "*.wikipedia.org"
    - "*.stackoverflow.com"
    - "*.github.com"
    - "docs.*"
    - "*.python.org"
    - "*.rust-lang.org"

  blocked_domains:
    - "*.onion"
    - "pastebin.com"
    - "*.xxx"
    - "*.torrent"

  rate_limits:
    requests_per_minute: 10
    requests_per_hour: 100
    concurrent_requests: 3

  content_limits:
    max_response_size_bytes: 5242880  # 5MB
    max_pages_per_search: 10
    timeout_seconds: 10

  content_filtering:
    block_adult_content: true
    block_malware_sites: true
    redact_pii: true
    redact_patterns:
      - "\\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Z|a-z]{2,}\\b"  # Email
      - "\\b\\d{3}-\\d{2}-\\d{4}\\b"  # SSN
      - "\\b\\d{16}\\b"  # Credit card
```

#### 3.2.2 Implementation

```python
class WebSearchSandbox:
    def __init__(self, config: WebSearchConfig):
        self.config = config
        self.rate_limiter = TokenBucketRateLimiter(
            rate=config.requests_per_minute / 60,
            capacity=config.concurrent_requests
        )
        self.pii_redactor = PIIRedactor(config.redact_patterns)

    def search(self, query: str, num_results: int = 5) -> SearchResults:
        # Rate limit
        allowed, wait = self.rate_limiter.acquire()
        if not allowed:
            raise RateLimitError(f"Rate limited, retry in {wait:.1f}s")

        # Execute search
        raw_results = self._execute_search(query, num_results)

        # Filter and sanitize results
        filtered = []
        for result in raw_results:
            if not self._is_domain_allowed(result.url):
                continue

            # Fetch and filter content
            content = self._fetch_content(result.url)
            if content:
                sanitized = self.pii_redactor.redact(content)
                filtered.append(SearchResult(
                    url=result.url,
                    title=result.title,
                    snippet=sanitized[:500]
                ))

        return SearchResults(results=filtered[:num_results])

    def _is_domain_allowed(self, url: str) -> bool:
        domain = urlparse(url).netloc

        # Check blocklist first
        for pattern in self.config.blocked_domains:
            if fnmatch.fnmatch(domain, pattern):
                return False

        # Check allowlist if configured
        if self.config.allowed_domains:
            for pattern in self.config.allowed_domains:
                if fnmatch.fnmatch(domain, pattern):
                    return True
            return False

        return True
```

### 3.3 Database Access Sandbox

#### 3.3.1 Configuration

```yaml
database_sandbox:
  default_mode: read_only

  query_limits:
    timeout_seconds: 5
    max_rows: 10000
    max_result_size_bytes: 10485760  # 10MB
    max_joins: 5
    max_subqueries: 3

  permissions:
    default:
      allowed_operations: [SELECT]
      denied_tables: [users_credentials, api_keys, audit_logs]

    elevated:
      allowed_operations: [SELECT, INSERT, UPDATE]
      denied_tables: [users_credentials, api_keys]
      requires_approval: true

  row_level_security:
    enabled: true
    policies:
      - table: customer_data
        filter: "tenant_id = current_tenant()"
      - table: user_profiles
        filter: "user_id = current_user() OR role = 'admin'"
```

#### 3.3.2 Query Validation

```python
class DatabaseSandbox:
    def __init__(self, config: DatabaseConfig):
        self.config = config
        self.parser = SQLParser()

    def execute_query(self, query: str, params: dict = None) -> QueryResult:
        # Parse and validate query
        parsed = self.parser.parse(query)

        # Check operation type
        if parsed.operation not in self.config.allowed_operations:
            raise PermissionError(f"Operation {parsed.operation} not allowed")

        # Check table access
        for table in parsed.tables:
            if table in self.config.denied_tables:
                raise PermissionError(f"Access to table {table} denied")

        # Check complexity
        if parsed.join_count > self.config.max_joins:
            raise ValidationError(f"Too many joins: {parsed.join_count}")

        if parsed.subquery_count > self.config.max_subqueries:
            raise ValidationError(f"Too many subqueries")

        # Apply row-level security
        secured_query = self._apply_rls(parsed)

        # Execute with timeout
        return self._execute_with_limits(secured_query, params)

    def _execute_with_limits(self, query: str, params: dict) -> QueryResult:
        with self.connection.cursor() as cursor:
            # Set statement timeout
            cursor.execute(f"SET statement_timeout = '{self.config.timeout_seconds}s'")

            # Execute query
            cursor.execute(query, params)

            # Fetch with row limit
            rows = cursor.fetchmany(self.config.max_rows + 1)
            truncated = len(rows) > self.config.max_rows

            if truncated:
                rows = rows[:self.config.max_rows]

            return QueryResult(
                rows=rows,
                truncated=truncated,
                row_count=len(rows)
            )
```

---

## 4. Secret Management

### 4.1 Storage

#### 4.1.1 HashiCorp Vault (MVP)

```hcl
# Vault configuration
storage "raft" {
  path = "/vault/data"
  node_id = "vault-1"
}

listener "tcp" {
  address = "0.0.0.0:8200"
  tls_cert_file = "/vault/certs/vault.crt"
  tls_key_file = "/vault/certs/vault.key"
}

seal "awskms" {
  region = "us-west-2"
  kms_key_id = "alias/vault-unseal"
}

api_addr = "https://vault.apex.internal:8200"
cluster_addr = "https://vault.apex.internal:8201"
```

#### 4.1.2 Secret Paths

```
secret/
  apex/
    api-keys/
      openai          # OpenAI API key
      anthropic       # Anthropic API key
      google          # Google API credentials
    database/
      postgres/
        main          # Main database credentials
        readonly      # Read-only replica credentials
    services/
      redis           # Redis password
      rabbitmq        # RabbitMQ credentials
    agents/
      {agent-id}/     # Per-agent secrets
        credentials
        tokens
```

#### 4.1.3 Cloud KMS (Production)

```yaml
# AWS KMS configuration
kms:
  key_alias: "alias/apex-secrets"
  key_rotation: enabled
  key_policy:
    - principal: "arn:aws:iam::ACCOUNT:role/apex-service"
      actions:
        - "kms:Decrypt"
        - "kms:GenerateDataKey"
    - principal: "arn:aws:iam::ACCOUNT:role/apex-admin"
      actions:
        - "kms:*"

# Envelope encryption
encryption:
  algorithm: AES-256-GCM
  key_derivation: HKDF-SHA256
  data_key_caching:
    enabled: true
    max_age_seconds: 300
    max_messages: 1000
```

### 4.2 Access Control

#### 4.2.1 Token Policy

```hcl
# Vault policy for agents
path "secret/data/apex/api-keys/*" {
  capabilities = ["read"]
}

path "secret/data/apex/agents/{{identity.entity.id}}/*" {
  capabilities = ["read", "create", "update"]
}

path "secret/data/apex/database/readonly" {
  capabilities = ["read"]
}

# Token configuration
token {
  ttl = "1h"
  max_ttl = "4h"
  renewable = true
  num_uses = 0
}
```

#### 4.2.2 Access Implementation

```python
class SecretManager:
    def __init__(self, vault_addr: str):
        self.client = hvac.Client(url=vault_addr)
        self._authenticate()

    def get_secret(self, path: str, requester: str) -> dict:
        # Audit log the access
        self._audit_access(path, requester)

        # Check cache first
        cached = self._get_cached(path)
        if cached and not cached.expired:
            return cached.value

        # Fetch from Vault
        try:
            response = self.client.secrets.kv.v2.read_secret_version(
                path=path,
                raise_on_deleted_version=True
            )
            secret = response['data']['data']

            # Cache with short TTL
            self._cache_secret(path, secret, ttl=300)

            return secret

        except hvac.exceptions.Forbidden:
            raise PermissionError(f"Access denied to {path}")

    def _audit_access(self, path: str, requester: str):
        audit_log.info(
            "secret_accessed",
            path=path,
            requester=requester,
            timestamp=datetime.utcnow().isoformat()
        )
```

### 4.3 Logging Protection

#### 4.3.1 Redaction Patterns

```yaml
log_redaction:
  patterns:
    # API Keys
    - name: openai_key
      pattern: "sk-[a-zA-Z0-9]{48}"
      replacement: "[REDACTED:OPENAI_KEY]"

    - name: anthropic_key
      pattern: "sk-ant-[a-zA-Z0-9-]{95}"
      replacement: "[REDACTED:ANTHROPIC_KEY]"

    - name: generic_api_key
      pattern: "(?i)(api[_-]?key|apikey)[\"']?\\s*[:=]\\s*[\"']?([a-zA-Z0-9_-]{20,})"
      replacement: "$1=[REDACTED]"

    # Passwords
    - name: password
      pattern: "(?i)(password|passwd|pwd)[\"']?\\s*[:=]\\s*[\"']?([^\\s\"']+)"
      replacement: "$1=[REDACTED]"

    # Connection strings
    - name: connection_string
      pattern: "(?i)(postgres|mysql|mongodb)://[^:]+:([^@]+)@"
      replacement: "$1://[USER]:[REDACTED]@"

    # AWS credentials
    - name: aws_access_key
      pattern: "AKIA[0-9A-Z]{16}"
      replacement: "[REDACTED:AWS_ACCESS_KEY]"

    - name: aws_secret_key
      pattern: "(?i)(aws_secret_access_key|secret_key)[\"']?\\s*[:=]\\s*[\"']?([a-zA-Z0-9/+=]{40})"
      replacement: "$1=[REDACTED]"
```

#### 4.3.2 Implementation

```python
class LogRedactor:
    def __init__(self, patterns: list[RedactionPattern]):
        self.patterns = [
            (re.compile(p.pattern), p.replacement)
            for p in patterns
        ]

    def redact(self, message: str) -> str:
        result = message
        for pattern, replacement in self.patterns:
            result = pattern.sub(replacement, result)
        return result

class SecureLogger:
    def __init__(self, redactor: LogRedactor):
        self.redactor = redactor
        self.logger = logging.getLogger("apex")

    def info(self, message: str, **kwargs):
        safe_message = self.redactor.redact(message)
        safe_kwargs = {
            k: self.redactor.redact(str(v))
            for k, v in kwargs.items()
        }
        self.logger.info(safe_message, extra=safe_kwargs)
```

### 4.4 Rotation

#### 4.4.1 Rotation Policy

```yaml
rotation_policies:
  api_keys:
    interval: 30d
    overlap_period: 1h      # Both keys valid during transition
    notification: 7d        # Warn before rotation

  database_passwords:
    interval: 90d
    overlap_period: 5m
    notification: 14d

  service_tokens:
    interval: 7d
    overlap_period: 30m
    notification: 1d
```

#### 4.4.2 Zero-Downtime Rotation

```python
class SecretRotator:
    async def rotate_secret(self, path: str, generator: SecretGenerator):
        # Generate new secret
        new_secret = generator.generate()

        # Store as pending version
        await self.vault.write(
            f"{path}/pending",
            value=new_secret,
            metadata={"status": "pending", "created": datetime.utcnow()}
        )

        # Update external system (e.g., database password)
        await self._update_external_system(path, new_secret)

        # Promote pending to active
        old_secret = await self.vault.read(f"{path}/active")
        await self.vault.write(f"{path}/active", value=new_secret)
        await self.vault.write(f"{path}/previous", value=old_secret)

        # Keep previous valid for overlap period
        await asyncio.sleep(self.config.overlap_period)

        # Revoke previous
        await self._revoke_external(path, old_secret)
        await self.vault.delete(f"{path}/previous")
        await self.vault.delete(f"{path}/pending")

        # Audit
        audit_log.info(
            "secret_rotated",
            path=path,
            timestamp=datetime.utcnow()
        )
```

---

## 5. Audit Trail

### 5.1 What to Log

#### 5.1.1 Event Categories

| Category | Events | Retention |
|----------|--------|-----------|
| Agent Actions | Tool calls, decisions, state changes | 1 year |
| Human Approvals | Approve/deny, policy changes | 7 years |
| Authentication | Login, logout, token refresh | 1 year |
| Authorization | Permission checks, denials | 1 year |
| Configuration | Setting changes, deployments | 7 years |
| Security | Violations, alerts, incidents | 7 years |
| Resource Usage | Costs, tokens, API calls | 1 year |

#### 5.1.2 Required Fields

```python
@dataclass
class AuditEvent:
    # Required fields
    timestamp: datetime          # ISO 8601 UTC
    event_type: str             # Enumerated event type
    event_id: UUID              # Unique event identifier

    # Actor information
    actor_type: Literal["agent", "human", "system"]
    actor_id: str
    actor_name: Optional[str]

    # Action details
    action: str                 # What was done
    target_type: Optional[str]  # What it was done to
    target_id: Optional[str]

    # Context
    trace_id: str               # Distributed trace ID
    span_id: str                # Span ID
    parent_span_id: Optional[str]

    # Request context
    ip_address: Optional[str]
    user_agent: Optional[str]
    request_id: Optional[str]

    # Result
    result: Literal["success", "failure", "pending"]
    error_code: Optional[str]
    error_message: Optional[str]

    # Additional data
    parameters: dict            # Sanitized parameters
    metadata: dict              # Additional context
```

### 5.2 Log Format

#### 5.2.1 JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["timestamp", "event_type", "event_id", "actor_type", "actor_id", "action", "result", "trace_id"],
  "properties": {
    "timestamp": {
      "type": "string",
      "format": "date-time",
      "description": "ISO 8601 UTC timestamp"
    },
    "event_type": {
      "type": "string",
      "enum": [
        "TOOL_EXECUTED",
        "TASK_CREATED",
        "TASK_COMPLETED",
        "APPROVAL_REQUESTED",
        "APPROVAL_GRANTED",
        "APPROVAL_DENIED",
        "AUTH_LOGIN",
        "AUTH_LOGOUT",
        "AUTH_FAILED",
        "CONFIG_CHANGED",
        "SECRET_ACCESSED",
        "LIMIT_EXCEEDED",
        "CIRCUIT_BREAKER_TRIPPED",
        "AGENT_SPAWNED",
        "AGENT_TERMINATED"
      ]
    },
    "event_id": {
      "type": "string",
      "format": "uuid"
    },
    "actor_type": {
      "type": "string",
      "enum": ["agent", "human", "system"]
    },
    "actor_id": {
      "type": "string"
    },
    "action": {
      "type": "string"
    },
    "target_type": {
      "type": "string"
    },
    "target_id": {
      "type": "string"
    },
    "result": {
      "type": "string",
      "enum": ["success", "failure", "pending"]
    },
    "trace_id": {
      "type": "string"
    },
    "parameters": {
      "type": "object"
    },
    "metadata": {
      "type": "object"
    }
  }
}
```

#### 5.2.2 Example Events

```json
{
  "timestamp": "2026-01-29T15:30:00.123Z",
  "event_type": "TOOL_EXECUTED",
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "actor_type": "agent",
  "actor_id": "agent-abc123",
  "actor_name": "ResearchAgent",
  "action": "web_search",
  "target_type": "external_api",
  "target_id": "google_search",
  "result": "success",
  "trace_id": "trace-xyz789",
  "span_id": "span-001",
  "parameters": {
    "query": "Python async best practices",
    "num_results": 5
  },
  "metadata": {
    "tokens_used": 150,
    "latency_ms": 342,
    "result_count": 5
  }
}
```

```json
{
  "timestamp": "2026-01-29T15:30:05.456Z",
  "event_type": "APPROVAL_REQUESTED",
  "event_id": "550e8400-e29b-41d4-a716-446655440001",
  "actor_type": "agent",
  "actor_id": "agent-abc123",
  "action": "request_approval",
  "target_type": "tool",
  "target_id": "database_write",
  "result": "pending",
  "trace_id": "trace-xyz789",
  "span_id": "span-002",
  "parameters": {
    "operation": "INSERT",
    "table": "reports",
    "reason": "Store analysis results"
  },
  "metadata": {
    "approval_id": "approval-def456",
    "timeout": 3600
  }
}
```

### 5.3 Retention

#### 5.3.1 Storage Tiers

```yaml
retention:
  hot_storage:
    duration: 30d
    backend: elasticsearch
    features:
      - full_text_search
      - real_time_queries
      - aggregations

  warm_storage:
    duration: 1y
    backend: s3_standard
    features:
      - compressed
      - indexed_metadata
      - query_via_athena

  cold_storage:
    duration: 7y
    backend: s3_glacier
    features:
      - compressed
      - encrypted
      - compliance_hold

  deletion:
    policy: secure_delete
    verification: checksum
    audit: required
```

#### 5.3.2 Data Lifecycle

```python
class AuditRetentionManager:
    async def manage_lifecycle(self):
        # Hot -> Warm transition (daily)
        hot_cutoff = datetime.utcnow() - timedelta(days=30)
        await self._transition_to_warm(before=hot_cutoff)

        # Warm -> Cold transition (monthly)
        warm_cutoff = datetime.utcnow() - timedelta(days=365)
        await self._transition_to_cold(before=warm_cutoff)

        # Cold deletion (with compliance check)
        cold_cutoff = datetime.utcnow() - timedelta(days=2555)  # 7 years
        await self._delete_with_compliance_check(before=cold_cutoff)

    async def _transition_to_warm(self, before: datetime):
        # Export from Elasticsearch
        events = await self.elasticsearch.export(
            index="audit-*",
            query={"range": {"timestamp": {"lt": before.isoformat()}}}
        )

        # Compress and upload to S3
        compressed = self._compress(events)
        key = f"warm/{before.strftime('%Y/%m/%d')}/audit.json.gz"
        await self.s3.upload(key, compressed)

        # Create Athena partition
        await self.athena.add_partition(key)

        # Delete from hot storage
        await self.elasticsearch.delete_by_query(
            index="audit-*",
            query={"range": {"timestamp": {"lt": before.isoformat()}}}
        )
```

---

## 6. Authentication & Authorization

### 6.1 Human Users

#### 6.1.1 OAuth 2.0 / OIDC Configuration

```yaml
oauth:
  providers:
    google:
      client_id: "${GOOGLE_CLIENT_ID}"
      client_secret: "${GOOGLE_CLIENT_SECRET}"
      authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth"
      token_endpoint: "https://oauth2.googleapis.com/token"
      userinfo_endpoint: "https://openidconnect.googleapis.com/v1/userinfo"
      scopes: ["openid", "email", "profile"]

    github:
      client_id: "${GITHUB_CLIENT_ID}"
      client_secret: "${GITHUB_CLIENT_SECRET}"
      authorization_endpoint: "https://github.com/login/oauth/authorize"
      token_endpoint: "https://github.com/login/oauth/access_token"
      userinfo_endpoint: "https://api.github.com/user"
      scopes: ["read:user", "user:email"]

    enterprise:
      type: oidc
      issuer: "https://idp.company.com"
      client_id: "${ENTERPRISE_CLIENT_ID}"
      client_secret: "${ENTERPRISE_CLIENT_SECRET}"
      scopes: ["openid", "email", "profile", "groups"]
```

#### 6.1.2 MFA Requirements

```yaml
mfa:
  required_for:
    - role: admin
      always: true
    - action: config:modify
      always: true
    - action: secrets:manage
      always: true
    - action: agents:delete
      always: true

  methods:
    - type: totp
      issuer: "Apex"
      algorithm: SHA256
      digits: 6
      period: 30

    - type: webauthn
      rp_name: "Apex"
      rp_id: "apex.company.com"
      attestation: "direct"
      user_verification: "preferred"

  backup_codes:
    count: 10
    length: 8
    single_use: true
```

#### 6.1.3 Session Management

```python
class SessionManager:
    def __init__(self, config: SessionConfig):
        self.config = config
        self.jwt_secret = load_secret("jwt_signing_key")

    def create_session(self, user: User) -> Session:
        # Create access token (short-lived)
        access_token = jwt.encode(
            {
                "sub": user.id,
                "email": user.email,
                "roles": user.roles,
                "type": "access",
                "iat": datetime.utcnow(),
                "exp": datetime.utcnow() + timedelta(hours=1),
                "jti": str(uuid4())
            },
            self.jwt_secret,
            algorithm="HS256"
        )

        # Create refresh token (long-lived)
        refresh_token = jwt.encode(
            {
                "sub": user.id,
                "type": "refresh",
                "iat": datetime.utcnow(),
                "exp": datetime.utcnow() + timedelta(days=7),
                "jti": str(uuid4())
            },
            self.jwt_secret,
            algorithm="HS256"
        )

        # Store refresh token hash
        self._store_refresh_token(user.id, refresh_token)

        return Session(
            access_token=access_token,
            refresh_token=refresh_token,
            expires_in=3600
        )

    def refresh_session(self, refresh_token: str) -> Session:
        # Validate refresh token
        payload = jwt.decode(refresh_token, self.jwt_secret, algorithms=["HS256"])

        if payload["type"] != "refresh":
            raise InvalidTokenError("Not a refresh token")

        # Check if token is revoked
        if self._is_token_revoked(payload["jti"]):
            raise InvalidTokenError("Token revoked")

        # Rotate refresh token
        user = self._get_user(payload["sub"])
        self._revoke_token(payload["jti"])

        return self.create_session(user)
```

### 6.2 Agents

#### 6.2.1 API Key Authentication

```python
class AgentAuthenticator:
    def __init__(self):
        self.key_store = SecretManager()

    def create_agent_key(self, agent_id: str, scopes: list[str]) -> AgentKey:
        # Generate key
        key_id = f"ak_{secrets.token_hex(8)}"
        key_secret = secrets.token_urlsafe(32)

        # Hash for storage
        key_hash = hashlib.sha256(key_secret.encode()).hexdigest()

        # Store metadata
        self.key_store.write(
            f"agent_keys/{key_id}",
            {
                "agent_id": agent_id,
                "key_hash": key_hash,
                "scopes": scopes,
                "created_at": datetime.utcnow().isoformat(),
                "last_used": None,
                "active": True
            }
        )

        return AgentKey(
            key_id=key_id,
            key_secret=key_secret,  # Only returned once
            scopes=scopes
        )

    def authenticate(self, api_key: str) -> AgentIdentity:
        # Parse key
        if not api_key.startswith("ak_"):
            raise AuthenticationError("Invalid key format")

        key_id = api_key[:11]  # ak_ + 8 hex chars
        key_secret = api_key[12:]

        # Fetch and verify
        metadata = self.key_store.read(f"agent_keys/{key_id}")
        if not metadata or not metadata["active"]:
            raise AuthenticationError("Invalid or inactive key")

        expected_hash = metadata["key_hash"]
        actual_hash = hashlib.sha256(key_secret.encode()).hexdigest()

        if not secrets.compare_digest(expected_hash, actual_hash):
            raise AuthenticationError("Invalid key")

        # Update last used
        self._update_last_used(key_id)

        return AgentIdentity(
            agent_id=metadata["agent_id"],
            scopes=metadata["scopes"]
        )
```

### 6.3 RBAC Model

#### 6.3.1 Role Definitions

```yaml
roles:
  admin:
    description: "Full system access"
    permissions:
      - "*"
    inherits: []

  operator:
    description: "Manage agents and approve actions"
    permissions:
      - "agents:*"
      - "tasks:*"
      - "approvals:*"
      - "config:view"
      - "secrets:view"
      - "audit:view"
    inherits: []

  viewer:
    description: "Read-only access"
    permissions:
      - "agents:view"
      - "tasks:view"
      - "approvals:view"
      - "config:view"
      - "audit:view"
    inherits: []

  agent:
    description: "Agent execution permissions"
    permissions:
      - "tools:execute"
      - "tasks:update_own"
      - "approvals:request"
    inherits: []
    scoped: true  # Permissions scoped to own resources
```

#### 6.3.2 Permission Definitions

```yaml
permissions:
  # Agent management
  agents:create:
    description: "Create new agents"
    risk_level: medium
  agents:delete:
    description: "Delete agents"
    risk_level: high
    requires_mfa: true
  agents:modify:
    description: "Modify agent configuration"
    risk_level: medium
  agents:view:
    description: "View agent details"
    risk_level: low

  # Task management
  tasks:submit:
    description: "Submit new tasks"
    risk_level: medium
  tasks:cancel:
    description: "Cancel running tasks"
    risk_level: medium
  tasks:view:
    description: "View task details"
    risk_level: low

  # Approvals
  approvals:approve:
    description: "Approve pending actions"
    risk_level: high
  approvals:deny:
    description: "Deny pending actions"
    risk_level: medium
  approvals:view:
    description: "View approval requests"
    risk_level: low

  # Configuration
  config:modify:
    description: "Modify system configuration"
    risk_level: critical
    requires_mfa: true
  config:view:
    description: "View configuration"
    risk_level: low

  # Secrets
  secrets:manage:
    description: "Create, update, delete secrets"
    risk_level: critical
    requires_mfa: true
  secrets:view:
    description: "View secret metadata (not values)"
    risk_level: medium

  # Tools
  tools:execute:
    description: "Execute tools"
    risk_level: medium
    scoped: true
```

#### 6.3.3 Authorization Implementation

```python
class Authorizer:
    def __init__(self, rbac_config: RBACConfig):
        self.config = rbac_config
        self.permission_cache = TTLCache(maxsize=1000, ttl=300)

    def check_permission(
        self,
        actor: Actor,
        permission: str,
        resource: Optional[Resource] = None
    ) -> AuthorizationResult:

        # Build cache key
        cache_key = f"{actor.id}:{permission}:{resource.id if resource else '*'}"

        if cache_key in self.permission_cache:
            return self.permission_cache[cache_key]

        # Get actor's effective permissions
        effective_permissions = self._get_effective_permissions(actor)

        # Check wildcard
        if "*" in effective_permissions:
            result = AuthorizationResult(allowed=True)
            self.permission_cache[cache_key] = result
            return result

        # Check specific permission
        if permission not in effective_permissions:
            return AuthorizationResult(
                allowed=False,
                reason=f"Permission {permission} not granted"
            )

        # Check resource scope if applicable
        if resource and self.config.permissions[permission].get("scoped"):
            if not self._check_resource_scope(actor, resource):
                return AuthorizationResult(
                    allowed=False,
                    reason="Resource not in actor's scope"
                )

        # Check MFA requirement
        if self.config.permissions[permission].get("requires_mfa"):
            if not actor.mfa_verified:
                return AuthorizationResult(
                    allowed=False,
                    reason="MFA required for this action",
                    requires_mfa=True
                )

        result = AuthorizationResult(allowed=True)
        self.permission_cache[cache_key] = result
        return result

    def _get_effective_permissions(self, actor: Actor) -> set[str]:
        permissions = set()

        for role_name in actor.roles:
            role = self.config.roles[role_name]
            permissions.update(role["permissions"])

            # Handle inheritance
            for inherited in role.get("inherits", []):
                inherited_role = self.config.roles[inherited]
                permissions.update(inherited_role["permissions"])

        return permissions
```

---

## 7. Compliance

### 7.1 GDPR

#### 7.1.1 Data Subject Access Requests

```python
class DSARHandler:
    async def handle_export_request(self, user_id: str) -> ExportResult:
        """Handle data subject access request (export)."""

        # Collect all user data
        data = {
            "user_profile": await self.db.get_user(user_id),
            "tasks": await self.db.get_user_tasks(user_id),
            "audit_logs": await self.audit.get_user_logs(user_id),
            "approvals": await self.db.get_user_approvals(user_id),
            "preferences": await self.db.get_user_preferences(user_id),
        }

        # Remove internal fields
        sanitized = self._sanitize_for_export(data)

        # Generate export file
        export_file = self._generate_export(sanitized, format="json")

        # Log the request
        audit_log.info(
            "dsar_export",
            user_id=user_id,
            timestamp=datetime.utcnow()
        )

        return ExportResult(
            file=export_file,
            generated_at=datetime.utcnow(),
            expires_at=datetime.utcnow() + timedelta(days=7)
        )

    async def handle_deletion_request(self, user_id: str) -> DeletionResult:
        """Handle right to erasure request."""

        # Verify identity
        if not await self._verify_identity(user_id):
            raise IdentityVerificationError()

        # Check for retention requirements
        retention_holds = await self._check_retention_holds(user_id)
        if retention_holds:
            return DeletionResult(
                status="partial",
                reason="Some data retained for legal compliance",
                retained_categories=retention_holds
            )

        # Delete user data
        await self.db.delete_user_data(user_id)
        await self.cache.invalidate_user(user_id)

        # Anonymize audit logs (retain for compliance but anonymize)
        await self.audit.anonymize_user_logs(user_id)

        audit_log.info(
            "dsar_deletion",
            user_id="[DELETED]",
            timestamp=datetime.utcnow()
        )

        return DeletionResult(status="complete")
```

#### 7.1.2 Data Minimization

```yaml
data_minimization:
  collection:
    # Only collect what's necessary
    required_fields:
      - email
      - name
    optional_fields:
      - avatar_url
      - timezone
    never_collect:
      - date_of_birth
      - social_security_number
      - financial_information

  retention:
    # Automatically delete after period
    user_sessions: 30d
    task_results: 90d
    detailed_logs: 365d
    anonymized_analytics: 2y

  processing:
    # Minimize data in processing
    anonymize_for_ml: true
    aggregate_for_reporting: true
```

### 7.2 SOC 2

#### 7.2.1 Control Framework

```yaml
soc2_controls:
  # Security
  CC6.1:  # Logical and Physical Access Controls
    controls:
      - role_based_access
      - mfa_enforcement
      - session_management
    evidence:
      - access_logs
      - permission_matrices
      - mfa_enrollment_reports

  CC6.6:  # Encryption
    controls:
      - tls_1_3_minimum
      - encryption_at_rest
      - key_management
    evidence:
      - certificate_inventory
      - kms_audit_logs
      - encryption_policy

  CC7.2:  # System Monitoring
    controls:
      - audit_logging
      - anomaly_detection
      - alerting
    evidence:
      - audit_log_samples
      - alert_configurations
      - incident_reports

  # Availability
  A1.2:  # Disaster Recovery
    controls:
      - backup_procedures
      - recovery_testing
      - failover_mechanisms
    evidence:
      - backup_logs
      - recovery_test_results
      - rpo_rto_metrics

  # Confidentiality
  C1.1:  # Data Classification
    controls:
      - data_labeling
      - access_restrictions
      - encryption_requirements
    evidence:
      - data_inventory
      - classification_matrix
      - access_reviews
```

#### 7.2.2 Continuous Compliance

```python
class ComplianceMonitor:
    def __init__(self):
        self.controls = load_soc2_controls()
        self.evidence_collectors = {
            "access_logs": AccessLogCollector(),
            "permission_matrices": PermissionMatrixCollector(),
            "encryption_status": EncryptionStatusCollector(),
        }

    async def run_compliance_check(self) -> ComplianceReport:
        results = []

        for control_id, control in self.controls.items():
            # Check each control
            control_result = await self._check_control(control)
            results.append(control_result)

            # Collect evidence
            evidence = await self._collect_evidence(control)
            control_result.evidence = evidence

        # Generate report
        report = ComplianceReport(
            timestamp=datetime.utcnow(),
            results=results,
            overall_status=self._calculate_status(results)
        )

        # Store for auditors
        await self._store_report(report)

        return report
```

### 7.3 Audit Requirements

#### 7.3.1 Immutable Audit Logs

```python
class ImmutableAuditLog:
    def __init__(self, storage: Storage):
        self.storage = storage
        self.hash_chain = HashChain()

    async def append(self, event: AuditEvent):
        # Serialize event
        event_bytes = self._serialize(event)

        # Add to hash chain
        previous_hash = await self._get_last_hash()
        event_hash = self.hash_chain.add(event_bytes, previous_hash)

        # Store with hash
        record = ImmutableRecord(
            event=event,
            hash=event_hash,
            previous_hash=previous_hash,
            timestamp=datetime.utcnow()
        )

        # Write to append-only storage
        await self.storage.append(record)

        # Replicate to secondary storage
        await self._replicate(record)

    async def verify_integrity(self) -> IntegrityReport:
        """Verify the entire chain hasn't been tampered with."""
        records = await self.storage.read_all()

        violations = []
        previous_hash = None

        for record in records:
            # Verify hash
            computed_hash = self.hash_chain.compute_hash(
                self._serialize(record.event),
                record.previous_hash
            )

            if computed_hash != record.hash:
                violations.append(TamperViolation(
                    record=record,
                    expected_hash=computed_hash,
                    actual_hash=record.hash
                ))

            # Verify chain
            if previous_hash and record.previous_hash != previous_hash:
                violations.append(ChainViolation(
                    record=record,
                    expected_previous=previous_hash,
                    actual_previous=record.previous_hash
                ))

            previous_hash = record.hash

        return IntegrityReport(
            verified_records=len(records),
            violations=violations,
            integrity_intact=len(violations) == 0
        )
```

---

## 8. Incident Response

### 8.1 Detection

#### 8.1.1 Anomaly Detection Rules

```yaml
anomaly_detection:
  rules:
    - name: unusual_api_volume
      description: "Detect unusual API call volume"
      metric: api_calls_per_minute
      condition: "> 3 * rolling_average_1h"
      severity: medium
      action: alert

    - name: unusual_cost_spike
      description: "Detect cost spikes"
      metric: cost_per_minute
      condition: "> 5 * rolling_average_1h"
      severity: high
      action: [alert, throttle]

    - name: authentication_failures
      description: "Multiple auth failures"
      metric: auth_failures_per_ip
      condition: "> 10 in 5 minutes"
      severity: high
      action: [alert, block_ip]

    - name: data_exfiltration
      description: "Large data transfers"
      metric: egress_bytes_per_agent
      condition: "> 100MB in 10 minutes"
      severity: critical
      action: [alert, suspend_agent]

    - name: privilege_escalation
      description: "Attempts to access unauthorized resources"
      metric: authorization_denials_per_actor
      condition: "> 5 in 1 minute"
      severity: critical
      action: [alert, suspend_actor]
```

#### 8.1.2 Alert Configuration

```yaml
alerting:
  channels:
    - name: pagerduty
      type: pagerduty
      service_key: "${PAGERDUTY_KEY}"
      severity_mapping:
        critical: P1
        high: P2
        medium: P3

    - name: slack_security
      type: slack
      webhook: "${SLACK_SECURITY_WEBHOOK}"
      severities: [critical, high]

    - name: email_ops
      type: email
      recipients: ["security@company.com", "ops@company.com"]
      severities: [critical]

  escalation:
    - severity: critical
      immediate: [pagerduty, slack_security, email_ops]
      after_5m: [call_oncall]
      after_15m: [page_manager]

    - severity: high
      immediate: [slack_security]
      after_15m: [pagerduty]
      after_30m: [email_ops]
```

### 8.2 Response

#### 8.2.1 Kill Switch

```python
class KillSwitch:
    def __init__(self, db: Database, message_queue: MessageQueue):
        self.db = db
        self.mq = message_queue

    async def activate(self, reason: str, activated_by: str):
        """Emergency stop all agent operations."""

        # Log activation
        audit_log.critical(
            "kill_switch_activated",
            reason=reason,
            activated_by=activated_by,
            timestamp=datetime.utcnow()
        )

        # Set global flag
        await self.db.set("system:kill_switch", {
            "active": True,
            "reason": reason,
            "activated_by": activated_by,
            "activated_at": datetime.utcnow().isoformat()
        })

        # Broadcast stop signal to all agents
        await self.mq.broadcast(
            channel="agent:control",
            message={"type": "EMERGENCY_STOP", "reason": reason}
        )

        # Terminate all running tasks
        running_tasks = await self.db.get_running_tasks()
        for task in running_tasks:
            await self._terminate_task(task.id, reason="EMERGENCY_STOP")

        # Revoke all agent tokens
        await self._revoke_all_agent_tokens()

        # Alert operations team
        await self._send_emergency_alert(reason, activated_by)

    async def deactivate(self, deactivated_by: str):
        """Resume normal operations after incident resolution."""

        # Require MFA for deactivation
        if not await self._verify_mfa(deactivated_by):
            raise MFARequiredError()

        # Clear flag
        await self.db.delete("system:kill_switch")

        audit_log.info(
            "kill_switch_deactivated",
            deactivated_by=deactivated_by,
            timestamp=datetime.utcnow()
        )
```

#### 8.2.2 Isolation Procedures

```yaml
isolation_procedures:
  agent_isolation:
    steps:
      - revoke_api_key
      - terminate_running_tasks
      - block_tool_access
      - preserve_state_snapshot
      - quarantine_artifacts
    automated: true
    requires_approval: false

  user_isolation:
    steps:
      - revoke_sessions
      - disable_account
      - preserve_audit_logs
      - notify_user
    automated: false
    requires_approval: true
    approval_roles: [admin, security]

  network_isolation:
    steps:
      - update_firewall_rules
      - revoke_service_mesh_certificates
      - block_egress_traffic
    automated: false
    requires_approval: true
    approval_roles: [admin]
```

#### 8.2.3 Forensic Data Preservation

```python
class ForensicPreserver:
    async def preserve_incident_data(self, incident_id: str):
        """Preserve all data related to an incident for forensic analysis."""

        # Create preservation record
        preservation = PreservationRecord(
            incident_id=incident_id,
            created_at=datetime.utcnow(),
            status="in_progress"
        )

        # Collect data
        data = {
            "audit_logs": await self._collect_audit_logs(incident_id),
            "agent_states": await self._collect_agent_states(incident_id),
            "task_history": await self._collect_task_history(incident_id),
            "system_metrics": await self._collect_metrics(incident_id),
            "network_logs": await self._collect_network_logs(incident_id),
        }

        # Create tamper-evident package
        package = self._create_evidence_package(data)

        # Sign with timestamp authority
        signed_package = await self._sign_with_tsa(package)

        # Store in immutable storage
        storage_path = f"forensics/{incident_id}/{datetime.utcnow().isoformat()}"
        await self.immutable_storage.store(storage_path, signed_package)

        # Update preservation record
        preservation.status = "complete"
        preservation.storage_path = storage_path
        preservation.checksum = signed_package.checksum

        return preservation
```

### 8.3 Recovery

#### 8.3.1 Rollback Procedures

```yaml
rollback_procedures:
  configuration:
    method: restore_from_version_control
    steps:
      - identify_last_known_good
      - create_rollback_branch
      - apply_configuration
      - verify_system_health
      - update_documentation

  database:
    method: point_in_time_recovery
    steps:
      - identify_recovery_point
      - create_recovery_instance
      - verify_data_integrity
      - swap_database_connection
      - archive_corrupted_instance

  agent_state:
    method: checkpoint_restore
    steps:
      - identify_valid_checkpoint
      - verify_checkpoint_integrity
      - restore_agent_state
      - verify_agent_functionality
      - resume_operations
```

#### 8.3.2 State Restoration

```python
class StateRestorer:
    async def restore_from_checkpoint(
        self,
        agent_id: str,
        checkpoint_id: str
    ) -> RestorationResult:

        # Load checkpoint
        checkpoint = await self.checkpoint_store.load(checkpoint_id)

        # Verify integrity
        if not self._verify_checkpoint(checkpoint):
            raise CheckpointCorruptedError()

        # Stop agent
        await self.agent_manager.stop(agent_id)

        # Restore state
        await self.state_store.write(
            agent_id,
            checkpoint.state
        )

        # Restore context
        await self.context_store.write(
            agent_id,
            checkpoint.context
        )

        # Restart agent
        await self.agent_manager.start(agent_id)

        # Verify restoration
        verification = await self._verify_restoration(agent_id, checkpoint)

        return RestorationResult(
            success=verification.passed,
            checkpoint_id=checkpoint_id,
            restored_at=datetime.utcnow()
        )
```

#### 8.3.3 Post-Mortem Template

```markdown
# Incident Post-Mortem: [INCIDENT-ID]

## Summary
- **Date:** YYYY-MM-DD
- **Duration:** X hours Y minutes
- **Severity:** Critical/High/Medium/Low
- **Impact:** [Description of user/system impact]

## Timeline
| Time (UTC) | Event |
|------------|-------|
| HH:MM | [Event description] |

## Root Cause
[Detailed description of what caused the incident]

## Detection
- How was the incident detected?
- What alerts fired?
- Time to detection: X minutes

## Response
- Who responded?
- What actions were taken?
- What worked well?
- What could be improved?

## Resolution
- How was the incident resolved?
- Time to resolution: X hours

## Lessons Learned
1. [Lesson 1]
2. [Lesson 2]

## Action Items
| ID | Description | Owner | Due Date | Status |
|----|-------------|-------|----------|--------|
| 1  | [Action]    | [Name]| YYYY-MM-DD| Open   |

## Appendix
- [Link to relevant logs]
- [Link to metrics dashboards]
- [Link to related documentation]
```

---

## 9. Security Checklist

### Pre-Deployment Checklist

#### Infrastructure Security

- [ ] All secrets stored in Vault/KMS (no plaintext secrets in code or config)
- [ ] TLS 1.3 enabled for all external connections
- [ ] TLS 1.2+ for all internal connections
- [ ] No default credentials anywhere in the system
- [ ] Firewall rules configured and documented
- [ ] Network segmentation implemented
- [ ] Load balancer security headers configured

#### Authentication & Authorization

- [ ] OAuth/OIDC providers configured
- [ ] MFA enforced for admin accounts
- [ ] RBAC roles and permissions defined
- [ ] Agent API keys generated with minimal scopes
- [ ] Session management configured (timeouts, rotation)
- [ ] Password policy enforced (if applicable)

#### Audit & Logging

- [ ] Audit logging enabled for all actions
- [ ] Log redaction configured for sensitive data
- [ ] Log retention policies configured
- [ ] Immutable audit log storage configured
- [ ] Log forwarding to SIEM configured
- [ ] Alerting rules configured

#### Sandboxing & Isolation

- [ ] Code execution sandbox configured
- [ ] Web search sandbox configured
- [ ] Database access sandbox configured
- [ ] Network isolation verified
- [ ] Resource limits configured and tested

#### Data Protection

- [ ] Encryption at rest enabled (AES-256)
- [ ] Encryption in transit enabled (TLS)
- [ ] Backup encryption verified
- [ ] PII handling procedures documented
- [ ] Data retention policies configured

#### Incident Response

- [ ] Kill switch tested and documented
- [ ] Incident response plan documented
- [ ] Escalation procedures defined
- [ ] Forensic preservation procedures tested
- [ ] Recovery procedures tested
- [ ] Post-mortem template available

#### Compliance

- [ ] GDPR requirements addressed
- [ ] SOC 2 controls implemented
- [ ] Compliance monitoring configured
- [ ] Audit evidence collection automated
- [ ] Privacy policy updated

### Periodic Review Checklist (Monthly)

- [ ] Review access permissions and remove unused
- [ ] Rotate API keys and secrets
- [ ] Review audit logs for anomalies
- [ ] Test backup restoration
- [ ] Review and update security documentation
- [ ] Conduct security awareness training
- [ ] Review third-party dependencies for vulnerabilities

### Annual Review Checklist

- [ ] Penetration testing completed
- [ ] Security architecture review
- [ ] Disaster recovery drill
- [ ] Compliance audit
- [ ] Security policy review and update
- [ ] Vendor security assessment
- [ ] Insurance policy review

---

## Appendix A: Security Contacts

| Role | Name | Contact |
|------|------|---------|
| Security Lead | TBD | security@company.com |
| Incident Commander | TBD | incident@company.com |
| Compliance Officer | TBD | compliance@company.com |

## Appendix B: Related Documents

- [System Architecture](/docs/architecture/system-overview.md)
- [Deployment Guide](/docs/deployment/README.md)
- [API Documentation](/docs/api/README.md)
- [Incident Response Playbook](/docs/security/incident-response.md)

## Appendix C: Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-29 | Security Team | Initial document |
