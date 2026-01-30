# Project Apex - Quick Start Guide

Get up and running in 5 minutes.

## Prerequisites

- Docker & Docker Compose
- Rust 1.75+
- Python 3.11+
- Node.js 20+
- An OpenAI or Anthropic API key

## 1. Clone & Setup

```bash
git clone https://github.com/apex-swarm/apex.git
cd apex

# Run setup script
./scripts/setup.sh
```

## 2. Configure API Keys

Edit `.env` and add your API keys:

```bash
OPENAI_API_KEY=sk-your-key-here
# or
ANTHROPIC_API_KEY=sk-ant-your-key-here
```

## 3. Start Development

```bash
make dev
```

This starts:
- Rust API server on http://localhost:8080
- Python worker processes
- React dashboard on http://localhost:3000
- PostgreSQL, Redis, Jaeger, Prometheus, Grafana

## 4. Submit Your First Task

```bash
curl -X POST http://localhost:8080/api/v1/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Hello World",
    "instruction": "Say hello and tell me a fun fact",
    "limits": {
      "token_limit": 1000,
      "cost_limit": 0.01
    }
  }'
```

## 5. View the Dashboard

Open http://localhost:3000 to see:
- Real-time agent status
- Task progress
- Cost tracking
- System metrics

## Common Commands

| Command | Description |
|---------|-------------|
| `make dev` | Start development environment |
| `make test` | Run all tests |
| `make lint` | Run linters |
| `make build` | Build for production |
| `make health` | Check service health |
| `make docker-up` | Start infrastructure only |
| `make docker-down` | Stop all services |

## Next Steps

- Read the [Architecture Guide](docs/ARCHITECTURE.md)
- Explore the [API Documentation](docs/API.md)
- Deploy to production with [Deployment Guide](docs/DEPLOYMENT.md)

## Troubleshooting

### API not starting?
```bash
# Check logs
docker-compose logs -f api

# Verify database is ready
make health
```

### Workers not processing tasks?
```bash
# Check Redis connection
docker-compose exec redis redis-cli ping

# View worker logs
docker-compose logs -f worker
```

### Need help?
- Open an issue on GitHub
- Check existing issues for solutions
