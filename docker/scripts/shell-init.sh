#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Apex Development Container - Shell Initialization Script
# ══════════════════════════════════════════════════════════════════════════════

# ─────────────────────────────────────────────────────────────────────────────
# Environment Variables
# ─────────────────────────────────────────────────────────────────────────────

# Rust
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_HOME="$HOME/.cargo"
export RUSTUP_HOME="$HOME/.rustup"

# Node.js
export NVM_DIR="/usr/local/nvm"
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"

# Python
export PYENV_ROOT="/usr/local/pyenv"
export PATH="$PYENV_ROOT/shims:$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init -)"

# Go (if installed)
export GOPATH="$HOME/go"
export PATH="$GOPATH/bin:$PATH"

# Local binaries
export PATH="$HOME/.local/bin:$PATH"

# ─────────────────────────────────────────────────────────────────────────────
# Aliases
# ─────────────────────────────────────────────────────────────────────────────

# General
alias ll='ls -alF'
alias la='ls -A'
alias l='ls -CF'
alias ..='cd ..'
alias ...='cd ../..'

# Git
alias gs='git status'
alias ga='git add'
alias gc='git commit'
alias gp='git push'
alias gl='git log --oneline -20'
alias gd='git diff'
alias gco='git checkout'
alias gb='git branch'

# Docker
alias d='docker'
alias dc='docker compose'
alias dps='docker ps'
alias dlog='docker logs -f'
alias dexec='docker exec -it'

# Kubernetes
alias k='kubectl'
alias kgp='kubectl get pods'
alias kgs='kubectl get services'
alias kgd='kubectl get deployments'
alias klog='kubectl logs -f'

# Project Apex
alias apex-api='cargo run --manifest-path /workspace/src/backend/core/Cargo.toml'
alias apex-worker='python /workspace/src/backend/agents/main.py'
alias apex-dash='cd /workspace/src/frontend && npm run dev'
alias apex-test='make test'
alias apex-lint='make lint'
alias apex-fmt='make fmt'

# Development
alias c='cargo'
alias cw='cargo watch'
alias ct='cargo test'
alias cb='cargo build'
alias cr='cargo run'
alias py='python'
alias ipy='ipython'
alias n='npm'
alias nr='npm run'

# ─────────────────────────────────────────────────────────────────────────────
# Functions
# ─────────────────────────────────────────────────────────────────────────────

# Quick directory navigation
function mkcd() {
    mkdir -p "$1" && cd "$1"
}

# Git commit with message
function gcm() {
    git commit -m "$*"
}

# Docker compose up with rebuild
function dcup() {
    docker compose up -d --build "$@"
}

# Find and kill process on port
function killport() {
    lsof -ti:"$1" | xargs kill -9
}

# Quick grep in files
function grepf() {
    grep -rn "$1" "${2:-.}"
}

# Watch file changes and run command
function watchrun() {
    while inotifywait -r -e modify,create,delete .; do
        "$@"
    done
}

# Get pod logs in Kubernetes
function klogs() {
    kubectl logs -f "$(kubectl get pods | grep "$1" | head -1 | awk '{print $1}')"
}

# Database quick connect
function pgconnect() {
    psql "${DATABASE_URL:-postgres://apex:apex_secret@localhost:5432/apex}"
}

# Redis quick connect
function redisconnect() {
    redis-cli -u "${REDIS_URL:-redis://localhost:6379}"
}

# ─────────────────────────────────────────────────────────────────────────────
# Welcome Message
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                    Project Apex Development Container                 ║"
echo "╠══════════════════════════════════════════════════════════════════════╣"
echo "║  Rust:   $(rustc --version 2>/dev/null || echo 'not installed')        "
echo "║  Python: $(python --version 2>/dev/null || echo 'not installed')       "
echo "║  Node:   $(node --version 2>/dev/null || echo 'not installed')         "
echo "║  Docker: $(docker --version 2>/dev/null | cut -d' ' -f3 | tr -d ',' || echo 'not available')"
echo "╠══════════════════════════════════════════════════════════════════════╣"
echo "║  Commands:                                                            ║"
echo "║    apex-api    - Run API server                                       ║"
echo "║    apex-worker - Run agent worker                                     ║"
echo "║    apex-dash   - Run dashboard dev server                             ║"
echo "║    apex-test   - Run all tests                                        ║"
echo "║    apex-lint   - Run linters                                          ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
