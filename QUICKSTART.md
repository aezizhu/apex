# Apex - Quick Start

Get up and running in 2 minutes.

## Prerequisites

- Python 3.11+
- An Anthropic API key

## 1. Install

```bash
git clone https://github.com/aezizhu/apex.git
cd apex
pip install -r requirements.txt
```

## 2. Configure

```bash
cp .env.example .env
```

Edit `.env` and add your Anthropic API key:

```
ANTHROPIC_API_KEY=your-key-here
```

## 3. Run

```bash
python -m src.backend.core
```

Open **http://localhost:8000** in your browser.

## 4. Try It

- **Swarm mode** (default): Enter a research topic and watch agents work in parallel
- **Chat mode**: Click "Chat" in the header for direct conversation with Claude

## Optional: Docker

```bash
docker build -f src/backend/core/Dockerfile -t apex .
docker run -p 8000:8000 --env-file .env apex
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ANTHROPIC_API_KEY` | (required) | Anthropic API key |
| `FIRECRAWL_BASE_URL` | `https://api-production-91c7.up.railway.app` | Web search API |
| `APEX_HOST` | `0.0.0.0` | Bind address |
| `APEX_PORT` | `8000` | Port |
| `APEX_RELOAD` | `false` | Auto-reload for dev |

## Troubleshooting

### App won't start?
- Check that `ANTHROPIC_API_KEY` is set in `.env`
- Ensure Python 3.11+ is installed: `python --version`

### Swarm produces errors?
- Check the terminal for log output
- Verify your Anthropic API key is valid
- The Firecrawl search API must be reachable

## Next Steps

- Read the [Architecture Guide](docs/ARCHITECTURE.md)
- Check the [API Documentation](docs/API.md)
