# ═══════════════════════════════════════════════════════════════════════════════
# Apex Agent Runtime - Multi-Stage Dockerfile
# ═══════════════════════════════════════════════════════════════════════════════
#
# Build stages:
#   1. builder - Install dependencies and build wheels
#   2. runtime - Minimal production image
#
# Usage:
#   docker build -t apex-agents .
#   docker run -e APEX_LLM_OPENAI_API_KEY=sk-... apex-agents
#
# ═══════════════════════════════════════════════════════════════════════════════

# -----------------------------------------------------------------------------
# Stage 0: Development (for docker-compose override with hot-reload)
# -----------------------------------------------------------------------------
FROM python:3.11-slim as development

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONPATH=/app

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN pip install --no-cache-dir watchdog[watchmedo] debugpy

WORKDIR /app

COPY pyproject.toml ./
RUN pip install --no-cache-dir \
    httpx>=0.26.0 \
    pydantic>=2.5.0 \
    pydantic-settings>=2.0.0 \
    openai>=1.6.0 \
    anthropic>=0.8.0 \
    redis>=5.0.0 \
    opentelemetry-api>=1.22.0 \
    opentelemetry-sdk>=1.22.0 \
    opentelemetry-exporter-otlp>=1.22.0 \
    opentelemetry-instrumentation-httpx>=0.43b0 \
    structlog>=23.3.0 \
    tenacity>=8.2.0 \
    tiktoken>=0.5.0

COPY . .
RUN pip install --no-cache-dir .

EXPOSE 8081 5678

CMD ["python", "main.py"]

# -----------------------------------------------------------------------------
# Stage 1: Builder
# -----------------------------------------------------------------------------
FROM python:3.11-slim as builder

# Set build-time environment variables
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

# Create and set working directory
WORKDIR /build

# Copy dependency files first for better caching
COPY pyproject.toml ./

# Install hatch for building
RUN pip install --upgrade pip hatchling

# Install dependencies
RUN pip install --target=/install \
    httpx>=0.26.0 \
    pydantic>=2.5.0 \
    pydantic-settings>=2.0.0 \
    openai>=1.6.0 \
    anthropic>=0.8.0 \
    redis>=5.0.0 \
    opentelemetry-api>=1.22.0 \
    opentelemetry-sdk>=1.22.0 \
    opentelemetry-exporter-otlp>=1.22.0 \
    opentelemetry-instrumentation-httpx>=0.43b0 \
    structlog>=23.3.0 \
    tenacity>=8.2.0 \
    tiktoken>=0.5.0

# Copy source code
COPY apex_agents/ ./apex_agents/
COPY main.py ./

# Build the package
RUN pip install --target=/install .

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM python:3.11-slim as runtime

# Labels
LABEL org.opencontainers.image.title="Apex Agent Runtime" \
      org.opencontainers.image.description="Python agent runtime for Project Apex" \
      org.opencontainers.image.version="0.1.0" \
      org.opencontainers.image.authors="Aezi <aezi.zhu@icloud.com>" \
      org.opencontainers.image.source="https://github.com/apex-swarm/apex"

# Set runtime environment variables
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONPATH=/app \
    # Default Apex configuration
    APEX_ENVIRONMENT=production \
    APEX_LOG_JSON=true \
    APEX_LOG_LEVEL=INFO \
    APEX_WORKER_NUM_AGENTS=5 \
    APEX_WORKER_POLL_INTERVAL_SECONDS=1.0 \
    APEX_WORKER_HEARTBEAT_INTERVAL_SECONDS=10.0 \
    APEX_BACKEND_HOST=apex-core \
    APEX_BACKEND_HTTP_PORT=8080 \
    APEX_REDIS_URL=redis://redis:6379

# Install runtime dependencies only
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for security
RUN groupadd --gid 1000 apex \
    && useradd --uid 1000 --gid apex --shell /bin/bash --create-home apex

# Set working directory
WORKDIR /app

# Copy installed packages from builder
COPY --from=builder /install /usr/local/lib/python3.11/site-packages/

# Copy application code
COPY --from=builder /build/apex_agents/ ./apex_agents/
COPY --from=builder /build/main.py ./

# Set ownership
RUN chown -R apex:apex /app

# Switch to non-root user
USER apex

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

# Expose metrics port (optional)
EXPOSE 8081

# Default command
ENTRYPOINT ["python", "main.py"]

# Default arguments (can be overridden)
CMD ["--workers", "1"]
