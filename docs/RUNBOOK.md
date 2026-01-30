# Project Apex - Operations Runbook

> Actionable guide for on-call engineers managing Apex in production

## Table of Contents

1. [On-Call Procedures](#on-call-procedures)
2. [Incident Response](#incident-response)
3. [Common Operational Tasks](#common-operational-tasks)
4. [Monitoring Alerts and Responses](#monitoring-alerts-and-responses)
5. [Disaster Recovery](#disaster-recovery)
6. [Maintenance Windows](#maintenance-windows)
7. [Capacity Planning](#capacity-planning)

---

## On-Call Procedures

### Shift Handoff Checklist

Before starting your on-call shift:

- [ ] Review open incidents in PagerDuty/OpsGenie
- [ ] Check Grafana dashboards for current system health
- [ ] Review recent deployments (last 24 hours)
- [ ] Verify access to all required systems:
  - [ ] Kubernetes cluster (`kubectl get nodes`)
  - [ ] Grafana dashboards
  - [ ] PagerDuty/alerting system
  - [ ] Database access (read-only)
  - [ ] Slack/Teams incident channel
- [ ] Read handoff notes from previous on-call engineer
- [ ] Confirm escalation contacts are up to date

### On-Call Responsibilities

1. **Acknowledge alerts** within 5 minutes during business hours, 15 minutes after hours
2. **Triage incidents** using the severity matrix below
3. **Communicate status** in the #apex-incidents Slack channel
4. **Document actions** in the incident ticket
5. **Perform handoff** to next on-call engineer

### Daily On-Call Checks

```bash
# Morning health check (run at start of shift)
# 1. Check all pods are running
kubectl get pods -n apex

# 2. Check for recent restarts
kubectl get pods -n apex -o wide | awk '$4 > 0'

# 3. Verify HPA status
kubectl get hpa -n apex

# 4. Check API health
curl -s https://apex.yourdomain.com/health | jq .

# 5. Verify queue depth
kubectl exec -it deployment/apex-api -n apex -- \
  redis-cli -h redis LLEN apex:tasks:queue

# 6. Check recent error rates in Grafana
# Navigate to: Grafana > Apex Overview > Error Rate panel
```

### Escalation Contacts

| Role | Primary | Secondary | Escalation Time |
|------|---------|-----------|-----------------|
| On-Call Engineer | Current rotation | Next in rotation | Immediate |
| Team Lead | @team-lead | @backup-lead | 15 minutes |
| Platform Team | @platform-oncall | @platform-lead | 30 minutes |
| Security Team | @security-oncall | security@company.com | Immediate (security incidents) |
| Executive | @eng-director | @cto | P1 incidents only |

---

## Incident Response

### Severity Levels

| Severity | Definition | Response Time | Update Frequency | Examples |
|----------|------------|---------------|------------------|----------|
| **P1 - Critical** | Complete service outage or data loss | 5 minutes | Every 15 minutes | API down, data corruption, security breach |
| **P2 - High** | Major feature unavailable, significant user impact | 15 minutes | Every 30 minutes | Task execution failing, high error rate (>25%) |
| **P3 - Medium** | Degraded service, limited user impact | 1 hour | Every 2 hours | Elevated latency, single worker failure |
| **P4 - Low** | Minor issue, no immediate user impact | 4 hours | Daily | Cosmetic issues, non-critical alerts |

### Incident Response Workflow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        INCIDENT RESPONSE WORKFLOW                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. DETECT          2. TRIAGE           3. RESPOND         4. RESOLVE       │
│  ─────────          ─────────           ──────────         ──────────       │
│  Alert fires   →    Assess impact  →    Mitigate      →    Fix root cause  │
│  Acknowledge        Assign severity     Communicate        Document         │
│                     Page if needed      Update status      Post-mortem      │
│                                                                              │
│  Target: <5min      Target: <10min      Ongoing            Within 48hrs     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Escalation Paths

#### P1 - Critical Incidents

```
1. On-Call Engineer (Immediate)
        │
        │ (15 min no progress)
        ▼
2. Team Lead + Secondary On-Call
        │
        │ (30 min no progress)
        ▼
3. Platform Team + Engineering Director
        │
        │ (60 min no progress)
        ▼
4. Executive Escalation (CTO)
```

#### P2 - High Severity Incidents

```
1. On-Call Engineer (15 min response)
        │
        │ (30 min no progress)
        ▼
2. Team Lead
        │
        │ (1 hour no progress)
        ▼
3. Platform Team
```

#### P3/P4 - Medium/Low Severity Incidents

```
1. On-Call Engineer (handles during shift)
        │
        │ (if requires expertise)
        ▼
2. Create JIRA ticket, assign to appropriate team
```

### Communication Templates

#### Initial Incident Notification

```markdown
:rotating_light: **INCIDENT DECLARED** :rotating_light:

**Severity:** P[1/2/3/4]
**Title:** [Brief description]
**Impact:** [What users are experiencing]
**Detection Time:** [YYYY-MM-DD HH:MM UTC]
**Incident Commander:** @[username]

**Current Status:**
[What we know so far]

**Actions Being Taken:**
- [ ] [Action 1]
- [ ] [Action 2]

**Next Update:** [Time in UTC]

Incident Channel: #incident-[YYYYMMDD]-[short-name]
```

#### Status Update Template

```markdown
:information_source: **INCIDENT UPDATE** - [HH:MM UTC]

**Severity:** P[1/2/3/4]
**Status:** [Investigating/Identified/Monitoring/Resolved]
**Duration:** [X hours Y minutes]

**Summary:**
[What has changed since last update]

**Metrics:**
- Error rate: [X%] (was [Y%])
- Latency P95: [X]ms (was [Y]ms)
- Affected users: ~[N]

**Actions Completed:**
- [Action 1]
- [Action 2]

**Next Steps:**
- [ ] [Next action]

**Next Update:** [Time in UTC]
```

#### Resolution Notification

```markdown
:white_check_mark: **INCIDENT RESOLVED**

**Severity:** P[1/2/3/4]
**Title:** [Brief description]
**Duration:** [Total time from detection to resolution]
**Resolution Time:** [YYYY-MM-DD HH:MM UTC]

**Root Cause:**
[Brief description of what caused the incident]

**Resolution:**
[What was done to fix it]

**Impact Summary:**
- Users affected: ~[N]
- Requests failed: ~[N]
- Downtime: [X minutes]

**Follow-up Actions:**
- [ ] Post-mortem scheduled for [date]
- [ ] [Preventive action 1]
- [ ] [Preventive action 2]

Post-mortem document: [Link]
```

---

## Common Operational Tasks

### Deploying New Versions

#### Standard Deployment (Helm)

```bash
# 1. Verify current state
kubectl get pods -n apex
helm list -n apex

# 2. Review changes
helm diff upgrade apex ./infra/k8s/helm/apex \
  --namespace apex \
  --values custom-values.yaml

# 3. Deploy (with --atomic for auto-rollback on failure)
helm upgrade apex ./infra/k8s/helm/apex \
  --namespace apex \
  --values custom-values.yaml \
  --atomic \
  --timeout 10m

# 4. Verify deployment
kubectl rollout status deployment/apex-api -n apex
kubectl rollout status deployment/apex-worker -n apex

# 5. Smoke test
curl -s https://apex.yourdomain.com/health | jq .
curl -s https://apex.yourdomain.com/ready | jq .
```

#### Canary Deployment

```bash
# 1. Deploy canary (10% traffic)
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: apex-api-canary
  namespace: apex
spec:
  replicas: 1
  selector:
    matchLabels:
      app: apex-api
      track: canary
  template:
    metadata:
      labels:
        app: apex-api
        track: canary
    spec:
      containers:
      - name: api
        image: apex/api:NEW_VERSION
        # ... rest of spec
EOF

# 2. Monitor canary metrics in Grafana for 15-30 minutes
# Compare error rates and latency between stable and canary

# 3. If successful, proceed with full rollout
helm upgrade apex ./infra/k8s/helm/apex \
  --namespace apex \
  --set api.image.tag=NEW_VERSION

# 4. Remove canary
kubectl delete deployment apex-api-canary -n apex
```

### Rolling Back Deployments

#### Helm Rollback

```bash
# 1. List deployment history
helm history apex -n apex

# Output example:
# REVISION  STATUS      DESCRIPTION
# 1         superseded  Install complete
# 2         superseded  Upgrade complete
# 3         deployed    Upgrade complete

# 2. Rollback to previous version
helm rollback apex 2 -n apex

# 3. Verify rollback
kubectl rollout status deployment/apex-api -n apex

# 4. Confirm service health
curl -s https://apex.yourdomain.com/health | jq .
```

#### Kubernetes Rollback (if Helm unavailable)

```bash
# Rollback API deployment
kubectl rollout undo deployment/apex-api -n apex

# Rollback to specific revision
kubectl rollout undo deployment/apex-api -n apex --to-revision=2

# Check rollback status
kubectl rollout status deployment/apex-api -n apex
```

#### Emergency Rollback Script

```bash
#!/bin/bash
# emergency-rollback.sh
# Usage: ./emergency-rollback.sh [component] [revision]

COMPONENT=${1:-"all"}
REVISION=${2:-""}

echo "Starting emergency rollback for: $COMPONENT"

if [[ "$COMPONENT" == "all" || "$COMPONENT" == "api" ]]; then
    echo "Rolling back apex-api..."
    if [[ -n "$REVISION" ]]; then
        kubectl rollout undo deployment/apex-api -n apex --to-revision=$REVISION
    else
        kubectl rollout undo deployment/apex-api -n apex
    fi
fi

if [[ "$COMPONENT" == "all" || "$COMPONENT" == "worker" ]]; then
    echo "Rolling back apex-worker..."
    if [[ -n "$REVISION" ]]; then
        kubectl rollout undo deployment/apex-worker -n apex --to-revision=$REVISION
    else
        kubectl rollout undo deployment/apex-worker -n apex
    fi
fi

# Wait for rollouts to complete
kubectl rollout status deployment/apex-api -n apex --timeout=300s
kubectl rollout status deployment/apex-worker -n apex --timeout=300s

# Verify health
echo "Verifying health..."
sleep 10
curl -s https://apex.yourdomain.com/health | jq .

echo "Rollback complete!"
```

### Scaling Services

#### Manual Scaling

```bash
# Scale API pods
kubectl scale deployment/apex-api --replicas=5 -n apex

# Scale worker pods
kubectl scale deployment/apex-worker --replicas=10 -n apex

# Verify scaling
kubectl get pods -n apex -l app=apex-api
kubectl get pods -n apex -l app=apex-worker
```

#### Adjusting HPA Settings

```bash
# View current HPA configuration
kubectl get hpa -n apex -o yaml

# Update HPA min/max replicas
kubectl patch hpa apex-api -n apex --patch '{"spec":{"minReplicas":5,"maxReplicas":30}}'

# Update target CPU utilization
kubectl patch hpa apex-api -n apex --patch '{"spec":{"targetCPUUtilizationPercentage":60}}'
```

#### Scaling for Expected Load

```bash
# Before high-traffic event (e.g., product launch)
# 1. Pre-scale workers
kubectl scale deployment/apex-worker --replicas=20 -n apex

# 2. Increase HPA limits
kubectl patch hpa apex-api -n apex --patch '{"spec":{"maxReplicas":50}}'
kubectl patch hpa apex-worker -n apex --patch '{"spec":{"maxReplicas":100}}'

# 3. Verify resources
kubectl top pods -n apex

# After event - return to normal
kubectl patch hpa apex-api -n apex --patch '{"spec":{"minReplicas":3,"maxReplicas":20}}'
kubectl patch hpa apex-worker -n apex --patch '{"spec":{"minReplicas":5,"maxReplicas":50}}'
```

### Database Maintenance

#### Running Migrations

```bash
# 1. Create a migration job
kubectl apply -f - <<EOF
apiVersion: batch/v1
kind: Job
metadata:
  name: apex-migration-$(date +%Y%m%d%H%M%S)
  namespace: apex
spec:
  template:
    spec:
      containers:
      - name: migration
        image: apex/api:latest
        command: ["./apex-migrate", "up"]
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: apex-secrets
              key: database-url
      restartPolicy: Never
  backoffLimit: 1
EOF

# 2. Monitor migration
kubectl logs -f job/apex-migration-* -n apex

# 3. Verify migration status
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT * FROM _sqlx_migrations ORDER BY version DESC LIMIT 5;"
```

#### Database Vacuum and Analyze

```bash
# Connect to database
kubectl exec -it statefulset/postgresql -n apex -- psql -U apex

# Run vacuum analyze on heavily-used tables
VACUUM ANALYZE tasks;
VACUUM ANALYZE agents;
VACUUM ANALYZE events;

# Check table statistics
SELECT relname, n_live_tup, n_dead_tup, last_vacuum, last_autovacuum
FROM pg_stat_user_tables
ORDER BY n_dead_tup DESC;
```

#### Checking Database Health

```bash
# Check connection count
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT count(*) FROM pg_stat_activity WHERE datname = 'apex';"

# Check long-running queries
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT pid, now() - pg_stat_activity.query_start AS duration, query
                   FROM pg_stat_activity
                   WHERE (now() - pg_stat_activity.query_start) > interval '5 minutes'
                   AND state != 'idle';"

# Check table sizes
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT relname, pg_size_pretty(pg_total_relation_size(relid))
                   FROM pg_catalog.pg_statio_user_tables
                   ORDER BY pg_total_relation_size(relid) DESC LIMIT 10;"
```

#### Terminating Stuck Queries

```bash
# Identify stuck query PID
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT pid, query FROM pg_stat_activity WHERE state = 'active';"

# Cancel query (graceful)
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT pg_cancel_backend(<PID>);"

# Terminate connection (forceful - use with caution)
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT pg_terminate_backend(<PID>);"
```

### Certificate Renewal

#### Checking Certificate Status

```bash
# Check certificate expiry
kubectl get certificate -n apex
kubectl describe certificate apex-tls -n apex

# Check actual certificate from ingress
echo | openssl s_client -servername apex.yourdomain.com -connect apex.yourdomain.com:443 2>/dev/null | \
  openssl x509 -noout -dates
```

#### Manual Certificate Renewal (cert-manager)

```bash
# Trigger certificate renewal
kubectl delete secret apex-tls -n apex
# cert-manager will automatically re-issue

# Or force renewal
kubectl annotate certificate apex-tls -n apex cert-manager.io/renew="true"

# Monitor renewal
kubectl describe certificate apex-tls -n apex
kubectl get certificaterequest -n apex
```

#### Manual Certificate Update (if not using cert-manager)

```bash
# 1. Update secret with new certificate
kubectl create secret tls apex-tls \
  --cert=/path/to/new/cert.pem \
  --key=/path/to/new/key.pem \
  --namespace apex \
  --dry-run=client -o yaml | kubectl apply -f -

# 2. Restart ingress controller to pick up new cert
kubectl rollout restart deployment/ingress-nginx-controller -n ingress-nginx

# 3. Verify new certificate
echo | openssl s_client -servername apex.yourdomain.com -connect apex.yourdomain.com:443 2>/dev/null | \
  openssl x509 -noout -dates
```

---

## Monitoring Alerts and Responses

### High Error Rate

**Alert:** `ApexHighTaskFailureRate`
**Condition:** Task failure rate > 10% for 5 minutes

#### Diagnosis

```bash
# 1. Check error rate trend in Grafana
# Dashboard: Apex Overview > Task Error Rate

# 2. Get recent errors from logs
kubectl logs deployment/apex-api -n apex --since=10m | grep -i error | tail -50

# 3. Check specific task failures
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT id, status, error_message, created_at
                   FROM tasks
                   WHERE status = 'failed'
                   AND created_at > NOW() - INTERVAL '1 hour'
                   ORDER BY created_at DESC LIMIT 20;"

# 4. Check LLM provider status
curl -s https://status.openai.com/api/v2/status.json | jq .
curl -s https://status.anthropic.com/api/v2/status.json | jq .
```

#### Response Actions

| Root Cause | Action |
|------------|--------|
| LLM provider outage | Enable fallback provider, notify stakeholders |
| Rate limiting from LLM | Reduce worker concurrency, check API quotas |
| Invalid API keys | Rotate keys, update secrets |
| Database connection issues | Check PostgreSQL health, connection pool |
| Worker crashes | Check worker logs, resource limits |

```bash
# Reduce worker concurrency (if rate limited)
kubectl set env deployment/apex-worker APEX__WORKER__CONCURRENCY=5 -n apex

# Switch to fallback LLM provider
kubectl set env deployment/apex-worker APEX__ROUTING__FORCE_PROVIDER=anthropic -n apex
```

### High Latency

**Alert:** `ApexHighLatency`
**Condition:** API P95 latency > 2s for 5 minutes

#### Diagnosis

```bash
# 1. Check latency breakdown in Jaeger
# Navigate to Jaeger UI > Search for slow traces

# 2. Check resource utilization
kubectl top pods -n apex

# 3. Check database query performance
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT calls, mean_exec_time, query
                   FROM pg_stat_statements
                   ORDER BY mean_exec_time DESC LIMIT 10;"

# 4. Check Redis latency
kubectl exec -it deployment/apex-api -n apex -- \
  redis-cli -h redis --latency-history
```

#### Response Actions

| Root Cause | Action |
|------------|--------|
| High CPU utilization | Scale up pods, check for resource-intensive tasks |
| Slow database queries | Analyze and optimize queries, add indexes |
| Redis latency | Check Redis memory, consider Redis cluster |
| Network issues | Check inter-pod connectivity, DNS resolution |
| External API delays | Increase timeouts, enable circuit breaker |

```bash
# Scale up API pods
kubectl scale deployment/apex-api --replicas=10 -n apex

# Increase connection pool (if database bottleneck)
kubectl set env deployment/apex-api APEX__DATABASE__MAX_CONNECTIONS=50 -n apex
```

### Resource Exhaustion

**Alert:** `ApexResourceExhaustion`
**Condition:** Pod memory > 90% or CPU > 90% sustained

#### Diagnosis

```bash
# 1. Check current resource usage
kubectl top pods -n apex

# 2. Check node resources
kubectl top nodes

# 3. Check for memory leaks (increasing memory over time)
kubectl logs deployment/apex-api -n apex | grep -i "memory\|oom"

# 4. Check pod resource limits
kubectl describe deployment/apex-api -n apex | grep -A5 "Limits\|Requests"
```

#### Response Actions

```bash
# Immediate: Scale horizontally
kubectl scale deployment/apex-api --replicas=8 -n apex

# If memory leak suspected: Rolling restart
kubectl rollout restart deployment/apex-api -n apex

# Increase resource limits (requires deployment update)
kubectl patch deployment apex-api -n apex --patch '
spec:
  template:
    spec:
      containers:
      - name: api
        resources:
          limits:
            memory: 4Gi
            cpu: 4
          requests:
            memory: 2Gi
            cpu: 1'
```

### Database Issues

**Alert:** `ApexDatabaseConnectionsHigh`
**Condition:** PostgreSQL connections > 80% of max

#### Diagnosis

```bash
# 1. Check connection count
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT count(*) as connections,
                          (SELECT setting FROM pg_settings WHERE name='max_connections') as max_connections
                   FROM pg_stat_activity WHERE datname = 'apex';"

# 2. Check connections by application
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT application_name, count(*)
                   FROM pg_stat_activity
                   WHERE datname = 'apex'
                   GROUP BY application_name;"

# 3. Check for idle connections
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT pid, state, query_start, query
                   FROM pg_stat_activity
                   WHERE datname = 'apex' AND state = 'idle'
                   ORDER BY query_start;"
```

#### Response Actions

```bash
# Terminate idle connections older than 10 minutes
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT pg_terminate_backend(pid)
                   FROM pg_stat_activity
                   WHERE datname = 'apex'
                   AND state = 'idle'
                   AND query_start < NOW() - INTERVAL '10 minutes';"

# Reduce connection pool size in applications
kubectl set env deployment/apex-api APEX__DATABASE__MAX_CONNECTIONS=20 -n apex
kubectl set env deployment/apex-worker APEX__DATABASE__MAX_CONNECTIONS=10 -n apex

# Increase PostgreSQL max_connections (requires restart)
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "ALTER SYSTEM SET max_connections = 200;"
kubectl rollout restart statefulset/postgresql -n apex
```

### Task Queue Backlog

**Alert:** `ApexTaskQueueBacklog`
**Condition:** Queue depth > 100 for 5+ minutes

#### Diagnosis

```bash
# 1. Check queue depth
kubectl exec -it deployment/apex-api -n apex -- \
  redis-cli -h redis LLEN apex:tasks:queue

# 2. Check worker status
kubectl get pods -n apex -l app=apex-worker

# 3. Check worker processing rate
kubectl logs deployment/apex-worker -n apex --since=5m | grep -c "task completed"

# 4. Check for stuck tasks
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT status, count(*)
                   FROM tasks
                   WHERE created_at > NOW() - INTERVAL '1 hour'
                   GROUP BY status;"
```

#### Response Actions

```bash
# Scale up workers
kubectl scale deployment/apex-worker --replicas=20 -n apex

# Check for and clear poisoned messages
kubectl exec -it deployment/apex-api -n apex -- \
  redis-cli -h redis LRANGE apex:tasks:dead-letter 0 10

# Clear dead letter queue if necessary (after investigation)
kubectl exec -it deployment/apex-api -n apex -- \
  redis-cli -h redis DEL apex:tasks:dead-letter
```

### Circuit Breaker Open

**Alert:** `ApexCircuitBreakerOpen`
**Condition:** Circuit breaker in OPEN state

#### Diagnosis

```bash
# 1. Check circuit breaker status
kubectl logs deployment/apex-api -n apex | grep "circuit_breaker"

# 2. Check which provider triggered the breaker
kubectl logs deployment/apex-api -n apex | grep -E "openai|anthropic|google" | tail -20

# 3. Check provider status pages
# OpenAI: https://status.openai.com
# Anthropic: https://status.anthropic.com
```

#### Response Actions

```bash
# Manual reset (after confirming provider is healthy)
curl -X POST http://apex-api:8080/admin/circuit-breaker/reset \
  -H "Authorization: Bearer $ADMIN_TOKEN"

# Switch to alternate provider
kubectl set env deployment/apex-worker APEX__ROUTING__FORCE_PROVIDER=anthropic -n apex

# Increase circuit breaker thresholds (if false positive)
kubectl set env deployment/apex-api APEX__CIRCUIT_BREAKER__FAILURE_THRESHOLD=10 -n apex
```

---

## Disaster Recovery

### Backup Procedures

#### Database Backup

```bash
# Manual backup
kubectl exec -it statefulset/postgresql -n apex -- \
  pg_dump -U apex -Fc apex > apex_backup_$(date +%Y%m%d_%H%M%S).dump

# Copy backup to local machine
kubectl cp apex/postgresql-0:/tmp/backup.dump ./backup.dump

# Automated backup (verify CronJob is running)
kubectl get cronjob -n apex
kubectl describe cronjob apex-db-backup -n apex
```

#### Backup Verification

```bash
# List available backups (S3 example)
aws s3 ls s3://apex-backups/database/ --recursive

# Test backup integrity
pg_restore --list apex_backup.dump

# Test restore to staging (recommended monthly)
# 1. Create test database
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "CREATE DATABASE apex_restore_test;"

# 2. Restore backup
kubectl exec -it statefulset/postgresql -n apex -- \
  pg_restore -U apex -d apex_restore_test apex_backup.dump

# 3. Verify data
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -d apex_restore_test -c "SELECT count(*) FROM tasks;"

# 4. Clean up
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "DROP DATABASE apex_restore_test;"
```

#### Redis Backup

```bash
# Trigger RDB snapshot
kubectl exec -it statefulset/redis -n apex -- redis-cli BGSAVE

# Check backup status
kubectl exec -it statefulset/redis -n apex -- redis-cli LASTSAVE

# Copy RDB file
kubectl cp apex/redis-0:/data/dump.rdb ./redis_backup_$(date +%Y%m%d).rdb
```

### Restore Procedures

#### Database Restore

```bash
# 1. Scale down applications to prevent writes
kubectl scale deployment/apex-api --replicas=0 -n apex
kubectl scale deployment/apex-worker --replicas=0 -n apex

# 2. Wait for connections to drain
sleep 30

# 3. Drop existing database and recreate
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U postgres -c "DROP DATABASE apex;"
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U postgres -c "CREATE DATABASE apex OWNER apex;"

# 4. Restore from backup
kubectl cp ./apex_backup.dump apex/postgresql-0:/tmp/backup.dump
kubectl exec -it statefulset/postgresql -n apex -- \
  pg_restore -U apex -d apex /tmp/backup.dump

# 5. Verify restore
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT count(*) FROM tasks;"

# 6. Scale applications back up
kubectl scale deployment/apex-api --replicas=3 -n apex
kubectl scale deployment/apex-worker --replicas=5 -n apex

# 7. Verify application health
kubectl rollout status deployment/apex-api -n apex
curl -s https://apex.yourdomain.com/health | jq .
```

#### Point-in-Time Recovery (if WAL archiving enabled)

```bash
# 1. Stop PostgreSQL
kubectl scale statefulset/postgresql --replicas=0 -n apex

# 2. Restore base backup
# (Implementation depends on your backup solution)

# 3. Create recovery.conf
kubectl exec -it statefulset/postgresql -n apex -- bash -c '
cat > /var/lib/postgresql/data/recovery.conf << EOF
restore_command = '\''cp /wal_archive/%f %p'\''
recovery_target_time = '\''2024-01-15 14:30:00'\''
recovery_target_action = '\''promote'\''
EOF'

# 4. Start PostgreSQL
kubectl scale statefulset/postgresql --replicas=1 -n apex
```

### Failover Procedures

#### Multi-Region Failover

```bash
# Prerequisites: Secondary region with warm standby

# 1. Verify secondary region health
kubectl --context=apex-secondary get pods -n apex

# 2. Promote secondary database
kubectl --context=apex-secondary exec -it statefulset/postgresql -n apex -- \
  pg_ctl promote -D /var/lib/postgresql/data

# 3. Update DNS to point to secondary region
# (This depends on your DNS provider)
# Example for AWS Route 53:
aws route53 change-resource-record-sets \
  --hosted-zone-id ZXXXXX \
  --change-batch file://failover-dns.json

# 4. Scale up secondary region workloads
kubectl --context=apex-secondary scale deployment/apex-api --replicas=5 -n apex
kubectl --context=apex-secondary scale deployment/apex-worker --replicas=10 -n apex

# 5. Verify traffic is flowing to secondary
curl -s https://apex.yourdomain.com/health | jq .
```

#### Single-Region Pod Failover

```bash
# Kubernetes handles this automatically via:
# - ReplicaSets maintaining desired pod count
# - Liveness/Readiness probes
# - HPA for load-based scaling

# Monitor automatic recovery
kubectl get events -n apex --sort-by='.lastTimestamp' | tail -20

# If pods not recovering, check node health
kubectl get nodes
kubectl describe node <problematic-node>

# Drain problematic node if necessary
kubectl drain <node-name> --ignore-daemonsets --delete-emptydir-data
```

---

## Maintenance Windows

### Scheduling Maintenance

| Maintenance Type | Window | Notification | Approval Required |
|------------------|--------|--------------|-------------------|
| Minor updates (patches) | Tuesday/Thursday 2-4 AM UTC | 24 hours | Team Lead |
| Major updates | Sunday 2-6 AM UTC | 1 week | Engineering Director |
| Emergency patches | Immediate | ASAP | On-Call Lead |
| Database maintenance | Sunday 4-6 AM UTC | 1 week | DBA + Team Lead |

### Pre-Maintenance Checklist

- [ ] Change request approved and documented
- [ ] Rollback plan prepared and tested
- [ ] Maintenance window communicated to stakeholders
- [ ] On-call engineer briefed
- [ ] Backups verified (within last 24 hours)
- [ ] Monitoring dashboards ready
- [ ] Status page updated (maintenance scheduled)

### Maintenance Procedures

#### Zero-Downtime Deployment

```bash
# 1. Announce maintenance start
# Update status page and notify #apex-announcements

# 2. Deploy with rolling update
helm upgrade apex ./infra/k8s/helm/apex \
  --namespace apex \
  --values custom-values.yaml \
  --set api.image.tag=NEW_VERSION \
  --atomic \
  --timeout 15m

# 3. Monitor rollout
watch kubectl get pods -n apex

# 4. Run smoke tests
./scripts/smoke-tests.sh

# 5. Monitor for 15 minutes
# Watch Grafana dashboards for anomalies

# 6. Announce maintenance complete
```

#### Maintenance Requiring Downtime

```bash
# 1. Announce maintenance start and expected duration
# Update status page to "Under Maintenance"

# 2. Enable maintenance mode (return 503 with message)
kubectl set env deployment/apex-api APEX__MAINTENANCE_MODE=true -n apex

# 3. Wait for in-flight requests to complete
sleep 60

# 4. Scale down workers
kubectl scale deployment/apex-worker --replicas=0 -n apex

# 5. Perform maintenance tasks
# ...

# 6. Scale workers back up
kubectl scale deployment/apex-worker --replicas=5 -n apex

# 7. Disable maintenance mode
kubectl set env deployment/apex-api APEX__MAINTENANCE_MODE=false -n apex

# 8. Verify health
curl -s https://apex.yourdomain.com/health | jq .

# 9. Announce maintenance complete
# Update status page to "Operational"
```

### Post-Maintenance Checklist

- [ ] All services healthy (health checks passing)
- [ ] Error rates within normal range
- [ ] Latency within normal range
- [ ] No alerts firing
- [ ] User-facing functionality verified
- [ ] Maintenance notes documented
- [ ] Status page updated
- [ ] Stakeholders notified of completion

---

## Capacity Planning

### Current Capacity Metrics

| Resource | Current | Threshold | Action Point |
|----------|---------|-----------|--------------|
| API Pods | 3-10 (HPA) | 80% of max | Increase HPA max |
| Worker Pods | 5-50 (HPA) | 80% of max | Increase HPA max |
| Database Connections | 100 max | 80 connections | Increase pool size |
| Database Storage | 100GB | 80% full | Expand volume |
| Redis Memory | 4GB | 80% used | Scale Redis |
| Node CPU | varies | 80% sustained | Add nodes |
| Node Memory | varies | 85% sustained | Add nodes |

### Monitoring Capacity

```bash
# Daily capacity check script
#!/bin/bash

echo "=== Apex Capacity Report $(date) ==="

echo -e "\n--- Pod Counts ---"
kubectl get pods -n apex --no-headers | wc -l
kubectl get hpa -n apex

echo -e "\n--- Node Resources ---"
kubectl top nodes

echo -e "\n--- Pod Resources ---"
kubectl top pods -n apex --sort-by=memory

echo -e "\n--- Database Stats ---"
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT pg_size_pretty(pg_database_size('apex')) as db_size;"
kubectl exec -it statefulset/postgresql -n apex -- \
  psql -U apex -c "SELECT count(*) as connections FROM pg_stat_activity WHERE datname = 'apex';"

echo -e "\n--- Redis Stats ---"
kubectl exec -it statefulset/redis -n apex -- redis-cli INFO memory | grep used_memory_human

echo -e "\n--- Queue Depth ---"
kubectl exec -it deployment/apex-api -n apex -- \
  redis-cli -h redis LLEN apex:tasks:queue
```

### Scaling Guidelines

#### When to Scale

| Metric | Warning | Critical | Action |
|--------|---------|----------|--------|
| API CPU | >70% sustained | >85% sustained | Increase replicas or limits |
| API Memory | >75% sustained | >90% sustained | Increase limits, check for leaks |
| Worker queue depth | >50 for 5min | >100 for 5min | Add workers |
| DB connections | >60% of max | >80% of max | Optimize queries, increase pool |
| DB storage | >70% full | >85% full | Expand volume, archive old data |

#### Scaling Procedures

```bash
# Increase HPA limits (proactive scaling)
kubectl patch hpa apex-api -n apex --patch '{
  "spec": {
    "minReplicas": 5,
    "maxReplicas": 30
  }
}'

kubectl patch hpa apex-worker -n apex --patch '{
  "spec": {
    "minReplicas": 10,
    "maxReplicas": 100
  }
}'

# Add cluster nodes (cloud provider dependent)
# AWS EKS
eksctl scale nodegroup \
  --cluster apex-cluster \
  --name standard-workers \
  --nodes 6

# GKE
gcloud container clusters resize apex-cluster \
  --zone us-central1-a \
  --num-nodes 6

# Expand database storage (AWS RDS example)
aws rds modify-db-instance \
  --db-instance-identifier apex-prod \
  --allocated-storage 200 \
  --apply-immediately
```

### Capacity Planning Calendar

| Quarter | Review Items | Actions |
|---------|--------------|---------|
| Q1 | Annual growth projection | Update resource budgets |
| Q2 | Mid-year capacity check | Adjust HPA limits |
| Q3 | Peak season preparation | Pre-scale for expected load |
| Q4 | Year-end review | Plan infrastructure for next year |

### Cost Optimization

```bash
# Check for over-provisioned resources
kubectl top pods -n apex | awk '$3 < "100m" {print "Low CPU:", $1}'
kubectl top pods -n apex | awk '$4 < "256Mi" {print "Low Memory:", $1}'

# Review HPA scaling events
kubectl describe hpa -n apex | grep -A5 "Events"

# Rightsize recommendations (if using tools like Goldilocks)
kubectl get vpa -n apex -o yaml
```

---

## Appendix

### Useful Commands Quick Reference

```bash
# Health checks
curl https://apex.yourdomain.com/health
curl https://apex.yourdomain.com/ready
curl https://apex.yourdomain.com/live

# Logs
kubectl logs -f deployment/apex-api -n apex
kubectl logs -f deployment/apex-worker -n apex
kubectl logs deployment/apex-api -n apex --previous  # crashed pod logs

# Resource status
kubectl get pods -n apex -o wide
kubectl get hpa -n apex
kubectl top pods -n apex

# Database
kubectl exec -it statefulset/postgresql -n apex -- psql -U apex

# Redis
kubectl exec -it deployment/apex-api -n apex -- redis-cli -h redis

# Debug
kubectl exec -it deployment/apex-api -n apex -- /bin/sh
kubectl port-forward svc/apex-api 8080:8080 -n apex
```

### Related Documentation

- [Architecture Overview](./ARCHITECTURE.md)
- [Deployment Guide](./DEPLOYMENT.md)
- [API Documentation](./API.md)

### Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2024-XX-XX | Apex Team | Initial runbook |
