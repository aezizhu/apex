.PHONY: help install dev test lint clean docker-build docker-run

help:
	@echo "Apex — Available Commands"
	@echo "═══════════════════════════════════"
	@echo ""
	@echo "  make install      - Install Python dependencies"
	@echo "  make dev          - Start development server with auto-reload"
	@echo "  make run          - Start production server"
	@echo "  make lint         - Run linters"
	@echo "  make clean        - Clean cached files"
	@echo ""
	@echo "  make docker-build - Build Docker image"
	@echo "  make docker-run   - Run in Docker"
	@echo ""

install:
	pip install -r requirements.txt

dev:
	APEX_RELOAD=true python -m src.backend.core

run:
	python -m src.backend.core

lint:
	python -m py_compile src/backend/core/app.py
	python -m py_compile src/backend/core/config.py
	python -m py_compile src/backend/core/swarm/engine.py

clean:
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name '*.pyc' -delete 2>/dev/null || true

docker-build:
	docker build -f src/backend/core/Dockerfile -t apex .

docker-run:
	docker run -p 8000:8000 --env-file .env apex
