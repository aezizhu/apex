<div align="center">

<h1>Apex</h1>

<p><strong>Autonomous Agent Swarm for Deep Research</strong></p>

<p>
A multi-agent research pipeline powered by MiniMax-M2.5 with real-time visualization.<br/>
Agents plan, research, analyze, fact-check, and write comprehensive reports autonomously.
</p>

</div>

---

## Features

- **Multi-Agent Pipeline** — Coordinator, Researchers, Analyst, Fact-Checker, and Writer agents collaborate autonomously
- **Live Web Research** — Agents search the web via Firecrawl API, with reflection cycles to fill gaps
- **Real-Time Dashboard** — WebSocket-powered UI shows agent activity, streaming output, and progress
- **Rich Report Generation** — Multi-stage report engine with templates, structured IR, and HTML rendering
- **Direct Chat** — Also supports direct conversation with MiniMax-M2.5

## Architecture

```
Browser (HTML/JS/CSS)
    │
    ├── WebSocket ──► Real-time swarm events
    └── HTTP ──────► REST API
                        │
              FastAPI (Python)
                        │
         ┌──────────────┼──────────────┐
         │              │              │
    SwarmEngine    LLMClient    WebSearchClient
         │              │              │
    Multi-agent    MiniMax API    Firecrawl API
    pipeline       (M2.5)        (web search)
```

## Quick Start

### Prerequisites

- Python 3.11+
- A [MiniMax API key](https://www.minimaxi.com/)

### Setup

```bash
# Clone the repo
git clone https://github.com/aezizhu/apex.git
cd apex

# Install dependencies
pip install -r requirements.txt

# Configure your API key
cp .env.example .env
# Edit .env and add your MINIMAX_API_KEY
```

### Run

```bash
python -m src.backend.core
```

The app starts at **http://localhost:8000**.

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MINIMAX_API_KEY` | (required) | Your MiniMax API key |
| `FIRECRAWL_BASE_URL` | `https://api-production-91c7.up.railway.app` | Firecrawl search API URL |
| `APEX_HOST` | `0.0.0.0` | Server bind address |
| `APEX_PORT` | `8000` | Server port |
| `APEX_RELOAD` | `false` | Enable auto-reload for development |

### Docker

```bash
docker build -f src/backend/core/Dockerfile -t apex .
docker run -p 8000:8000 --env-file .env apex
```

## How It Works

1. **Planning** — The Coordinator agent decomposes your query into a structured research plan with sections and tasks
2. **Researching** — Multiple Researcher agents search the web in parallel, with reflection cycles to identify and fill gaps
3. **Analyzing** — The Analyst synthesizes all research findings into a coherent analytical framework
4. **Fact-Checking** — The Fact-Checker verifies claims against original research data
5. **Writing** — A multi-stage report engine generates a structured, HTML-rendered report

## Project Structure

```
src/backend/core/
├── app.py              # FastAPI application + routes
├── config.py           # Configuration + env vars
├── __main__.py         # Entry point (uvicorn)
├── static/             # Frontend (HTML/JS/CSS)
│   ├── index.html
│   ├── app.js          # Chat mode
│   ├── swarm.js        # Swarm dashboard
│   ├── utils.js        # Shared utilities
│   └── style.css
└── swarm/
    ├── engine.py        # Swarm pipeline orchestrator
    ├── llm.py           # MiniMax API client
    ├── search.py        # Firecrawl web search client
    ├── models.py        # Data models
    ├── events.py        # WebSocket event emitter
    ├── bus.py           # In-memory message bus
    ├── roles.py         # Agent role configurations
    ├── report_engine.py # Multi-stage report pipeline
    ├── report_ir.py     # Document intermediate representation
    ├── report_stitcher.py
    ├── report_renderer.py
    └── report_templates/ # Markdown report templates
```

## License

Apache 2.0
