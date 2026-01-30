# Project Apex - Deployment Guide

> Complete guide for deploying Apex in production environments

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Local Development](#local-development)
3. [Docker Compose](#docker-compose)
4. [Kubernetes](#kubernetes)
5. [Cloud Providers](#cloud-providers)
6. [Configuration](#configuration)
7. [Monitoring](#monitoring)
8. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required Software

| Software | Version | Purpose |
|----------|---------|---------|
| Docker | 24.0+ | Container runtime |
| Docker Compose | 2.20+ | Local orchestration |
| Kubernetes | 1.28+ | Production orchestration |
| Helm | 3.12+ | K8s package management |
| kubectl | 1.28+ | K8s CLI |

### Required Credentials

- OpenAI API Key (for GPT models)
- Anthropic API Key (for Claude models)
- (Optional) Google Cloud credentials (for Gemini)

---

## Local Development

### Quick Start

```bash
# Clone the repository
git clone https://github.com/apex-swarm/apex.git
cd apex

# Copy environment template
cp .env.example .env

# Edit .env with your API keys
vim .env

# Start infrastructure only
docker-compose up -d postgres redis jaeger prometheus grafana

# Run the Rust backend
cd src/backend/core
cargo run

# In another terminal, run Python workers
cd src/backend/agents
pip install -e .
python main.py --workers 1

# In another terminal, run the frontend
cd src/frontend
npm install
npm run dev
```

### Access Points

| Service | URL | Credentials |
|---------|-----|-------------|
| Dashboard | http://localhost:3000 | - |
| API | http://localhost:8080 | - |
| Grafana | http://localhost:3001 | admin / apex_admin |
| Prometheus | http://localhost:9090 | - |
| Jaeger | http://localhost:16686 | - |

---

## Docker Compose

### Full Stack Deployment

```bash
# Start everything
docker-compose up -d

# View logs
docker-compose logs -f

# Stop everything
docker-compose down

# Stop and remove volumes (clean slate)
docker-compose down -v
```

### Production Docker Compose

Create `docker-compose.prod.yml`:

```yaml
version: '3.8'

services:
  api:
    image: apex/api:latest
    deploy:
      replicas: 2
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M
    environment:
      - RUST_LOG=info
      - DATABASE_URL=postgres://apex:${DB_PASSWORD}@postgres:5432/apex
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  worker:
    image: apex/worker:latest
    deploy:
      replicas: 3
      resources:
        limits:
          cpus: '1'
          memory: 1G
    environment:
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
```

---

## Kubernetes

### Helm Installation

```bash
# Add the Apex Helm repository (if published)
# helm repo add apex https://charts.apex-swarm.io
# helm repo update

# Or install from local charts
cd infra/k8s/helm

# Create namespace
kubectl create namespace apex

# Create secrets
kubectl create secret generic apex-secrets \
  --namespace apex \
  --from-literal=openai-api-key="${OPENAI_API_KEY}" \
  --from-literal=anthropic-api-key="${ANTHROPIC_API_KEY}" \
  --from-literal=database-url="postgres://apex:${DB_PASSWORD}@postgres:5432/apex" \
  --from-literal=redis-url="redis://redis:6379"

# Install with Helm
helm install apex ./apex \
  --namespace apex \
  --values ./apex/values.yaml \
  --set secrets.openaiApiKey="${OPENAI_API_KEY}" \
  --set secrets.anthropicApiKey="${ANTHROPIC_API_KEY}"
```

### Custom Values

Create `custom-values.yaml`:

```yaml
# Production overrides
api:
  replicaCount: 3
  resources:
    requests:
      memory: "1Gi"
      cpu: "1"
    limits:
      memory: "4Gi"
      cpu: "4"
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 20
    targetCPUUtilizationPercentage: 70

worker:
  replicaCount: 5
  workersPerPod: 2
  agentsPerWorker: 10
  autoscaling:
    enabled: true
    minReplicas: 5
    maxReplicas: 50

ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
  hosts:
    - host: apex.yourdomain.com
  tls:
    - secretName: apex-tls
      hosts:
        - apex.yourdomain.com

config:
  orchestrator:
    maxConcurrentAgents: "500"
    defaultTokenLimit: "50000"
    defaultCostLimit: "1.00"
```

Install with custom values:

```bash
helm install apex ./apex \
  --namespace apex \
  --values custom-values.yaml
```

### Upgrading

```bash
# Upgrade with new values
helm upgrade apex ./apex \
  --namespace apex \
  --values custom-values.yaml

# Rollback if needed
helm rollback apex 1 --namespace apex
```

### Monitoring the Deployment

```bash
# Check pod status
kubectl get pods -n apex

# View logs
kubectl logs -f deployment/apex-api -n apex

# Port forward for local access
kubectl port-forward svc/apex-api 8080:8080 -n apex

# Check HPA status
kubectl get hpa -n apex
```

---

## Cloud Providers

### AWS EKS

```bash
# Create EKS cluster
eksctl create cluster \
  --name apex-cluster \
  --region us-west-2 \
  --nodegroup-name standard-workers \
  --node-type t3.large \
  --nodes 3 \
  --nodes-min 2 \
  --nodes-max 10 \
  --managed

# Install AWS Load Balancer Controller
helm repo add eks https://aws.github.io/eks-charts
helm install aws-load-balancer-controller eks/aws-load-balancer-controller \
  --namespace kube-system \
  --set clusterName=apex-cluster

# Deploy Apex
helm install apex ./apex \
  --namespace apex \
  --set ingress.className=alb \
  --set ingress.annotations."kubernetes\.io/ingress\.class"=alb
```

### Google GKE

```bash
# Create GKE cluster
gcloud container clusters create apex-cluster \
  --zone us-central1-a \
  --num-nodes 3 \
  --machine-type e2-standard-4 \
  --enable-autoscaling \
  --min-nodes 2 \
  --max-nodes 10

# Get credentials
gcloud container clusters get-credentials apex-cluster --zone us-central1-a

# Deploy Apex
helm install apex ./apex --namespace apex
```

### Azure AKS

```bash
# Create AKS cluster
az aks create \
  --resource-group apex-rg \
  --name apex-cluster \
  --node-count 3 \
  --node-vm-size Standard_D4s_v3 \
  --enable-cluster-autoscaler \
  --min-count 2 \
  --max-count 10

# Get credentials
az aks get-credentials --resource-group apex-rg --name apex-cluster

# Deploy Apex
helm install apex ./apex --namespace apex
```

---

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | Required |
| `REDIS_URL` | Redis connection string | Required |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `RUST_LOG` | Rust log level | `info` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP endpoint | - |

### Orchestrator Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `APEX__ORCHESTRATOR__MAX_CONCURRENT_AGENTS` | Max parallel agents | `100` |
| `APEX__ORCHESTRATOR__DEFAULT_TOKEN_LIMIT` | Default token limit | `20000` |
| `APEX__ORCHESTRATOR__DEFAULT_COST_LIMIT` | Default cost limit | `0.25` |
| `APEX__ORCHESTRATOR__DEFAULT_TIME_LIMIT_SECONDS` | Default time limit | `300` |

### Model Router Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `APEX__ROUTING__ECONOMY_MODEL` | Economy tier model | `gpt-4o-mini` |
| `APEX__ROUTING__STANDARD_MODEL` | Standard tier model | `gpt-4o` |
| `APEX__ROUTING__PREMIUM_MODEL` | Premium tier model | `claude-3-opus` |
| `APEX__ROUTING__CONFIDENCE_THRESHOLD` | Cascade threshold | `0.85` |

---

## Monitoring

### Grafana Dashboards

Pre-configured dashboards are available at `http://grafana:3000`:

1. **Apex Overview** - High-level system health
2. **Agent Performance** - Individual agent metrics
3. **Task Analytics** - Task throughput and latency
4. **Cost Tracking** - LLM spending analysis

### Key Metrics

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `apex_active_agents` | Currently active agents | < 1 |
| `apex_task_queue_depth` | Pending tasks | > 100 |
| `apex_tasks_failed_total` | Failed task count | Rate > 10% |
| `apex_total_cost_used` | Total LLM cost | > 90% budget |
| `apex_api_request_duration_seconds` | API latency | P95 > 2s |

### Alerting

Alerts are defined in `prometheusrule.yaml`:

- **ApexTaskQueueBacklog** - Queue growing for 5+ minutes
- **ApexHighTaskFailureRate** - Failure rate > 10%
- **ApexNoActiveAgents** - No agents available
- **ApexCostLimitApproaching** - Cost > 90% of budget
- **ApexCircuitBreakerOpen** - Circuit breaker tripped

---

## Troubleshooting

### Common Issues

#### API Not Starting

```bash
# Check logs
kubectl logs -f deployment/apex-api -n apex

# Common causes:
# 1. Database not ready - wait for PostgreSQL
# 2. Missing migrations - run migrations
# 3. Invalid config - check environment variables
```

#### Workers Not Processing Tasks

```bash
# Check worker logs
kubectl logs -f deployment/apex-worker -n apex

# Check Redis connection
kubectl exec -it deployment/apex-worker -n apex -- redis-cli -h redis ping

# Common causes:
# 1. Redis connection failed
# 2. Invalid API keys
# 3. Rate limiting from LLM providers
```

#### High Latency

```bash
# Check resource utilization
kubectl top pods -n apex

# Scale up if needed
kubectl scale deployment/apex-api --replicas=5 -n apex

# Check database performance
kubectl exec -it statefulset/postgresql -n apex -- psql -U apex -c "SELECT * FROM pg_stat_activity"
```

#### Circuit Breaker Open

```bash
# Check circuit breaker status in logs
kubectl logs deployment/apex-api -n apex | grep "circuit_breaker"

# Manual reset (if needed)
curl -X POST http://apex-api:8080/admin/circuit-breaker/reset
```

### Health Checks

```bash
# API health
curl http://apex-api:8080/health

# Readiness
curl http://apex-api:8080/ready

# Liveness
curl http://apex-api:8080/live
```

### Debug Mode

Enable debug logging:

```bash
# Temporarily enable debug
kubectl set env deployment/apex-api RUST_LOG=debug -n apex

# View debug logs
kubectl logs -f deployment/apex-api -n apex

# Restore production logging
kubectl set env deployment/apex-api RUST_LOG=info -n apex
```
