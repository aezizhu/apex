# Contributing to Apex

First off, thank you for considering contributing to Apex! It's people like you that make Apex such a great tool. We welcome contributions from everyone, regardless of their level of experience.

This document provides guidelines and information about contributing to this project. Please read it carefully before making your contribution.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Code Style Guidelines](#code-style-guidelines)
- [Testing Requirements](#testing-requirements)
- [Pull Request Process](#pull-request-process)
- [Commit Message Conventions](#commit-message-conventions)
- [Issue Templates](#issue-templates)
- [Architecture Decision Records](#architecture-decision-records-adr)
- [Release Process](#release-process)
- [Getting Help](#getting-help)

---

## Code of Conduct

### Our Pledge

We are committed to providing a friendly, safe, and welcoming environment for all contributors. We pledge to make participation in our project and community a harassment-free experience for everyone, regardless of age, body size, disability, ethnicity, gender identity and expression, level of experience, nationality, personal appearance, race, religion, or sexual identity and orientation.

### Our Standards

**Examples of behavior that contributes to a positive environment:**

- Using welcoming and inclusive language
- Being respectful of differing viewpoints and experiences
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy towards other community members

**Examples of unacceptable behavior:**

- The use of sexualized language or imagery and unwelcome sexual attention
- Trolling, insulting/derogatory comments, and personal or political attacks
- Public or private harassment
- Publishing others' private information without explicit permission
- Other conduct which could reasonably be considered inappropriate

### Enforcement

Project maintainers are responsible for clarifying the standards of acceptable behavior and are expected to take appropriate and fair corrective action in response to any instances of unacceptable behavior.

For the full Code of Conduct, please see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

---

## Getting Started

### Finding Something to Work On

New to the project? Here are some ways to get started:

1. **Good First Issues**: Look for issues labeled [`good first issue`](../../labels/good%20first%20issue) - these are specifically curated for newcomers.

2. **Documentation**: Help us improve documentation, fix typos, or add examples.

3. **Bug Reports**: Help verify and reproduce reported bugs.

4. **Feature Requests**: Comment on feature requests with your thoughts or implementation ideas.

### Before You Begin

1. **Check existing issues**: Search the issue tracker to ensure your contribution hasn't been discussed before.

2. **Open an issue first**: For significant changes, please open an issue to discuss the proposed changes before starting work.

3. **Fork the repository**: Create your own fork to work on.

4. **Read the documentation**: Familiarize yourself with the project architecture and existing codebase.

---

## Development Setup

### Prerequisites

Ensure you have the following installed:

- **Git** (2.30+)
- **Rust** (1.75+ with cargo)
- **Python** (3.11+ with pip/poetry)
- **Node.js** (20+ with npm/yarn/pnpm)
- **Docker & Docker Compose**
- **PostgreSQL** (16+)

### Clone and Setup

```bash
# Clone the repository (or your fork)
git clone https://github.com/apex-swarm/apex.git
cd apex

# If working from a fork, add upstream remote
git remote add upstream https://github.com/apex-swarm/apex.git

# Copy environment file
cp .env.example .env

# Install all dependencies
make setup

# Start infrastructure services
make docker-up

# Start development servers
make dev
```

### Branch Naming Convention

Create a new branch for your work using the following naming conventions:

- `feature/description` - New features
- `fix/description` - Bug fixes
- `docs/description` - Documentation
- `refactor/description` - Code refactoring
- `test/description` - Test additions/fixes

```bash
# Create a new branch for your work
git checkout -b feature/your-feature-name
```

### Rust Setup

```bash
# Install Rust toolchain (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install required components
rustup component add clippy rustfmt

# Build the project
cargo build

# Run tests
cargo test
```

### Python Setup

```bash
# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt
pip install -r requirements-dev.txt

# Or using Poetry
poetry install
```

### TypeScript/Frontend Setup

```bash
# Install dependencies
npm install
# or
yarn install
# or
pnpm install

# Build the project
npm run build
```

### Environment Configuration

```bash
# Copy the example environment file
cp .env.example .env

# Edit .env with your local configuration
# Ensure database credentials and API keys are properly set
```

---

## Code Style Guidelines

Consistent code style makes the codebase more maintainable. We use automated tools to enforce style guidelines.

### Rust

We follow the official [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).

```bash
# Format code
cargo fmt

# Check for common mistakes
cargo clippy -- -D warnings
```

**Key guidelines:**

- Use `snake_case` for functions, methods, variables, and modules
- Use `PascalCase` for types and traits
- Use `SCREAMING_SNAKE_CASE` for constants
- Maximum line length: 100 characters
- Write doc comments for all public items
- Prefer `Result` and `Option` over panicking
- Use meaningful error messages with `thiserror` or `anyhow`

**Documentation format:**

```rust
/// Brief description of the function.
///
/// # Arguments
///
/// * `param` - Description of parameter
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// Description of error conditions
///
/// # Examples
///
/// ```
/// let result = function(param);
/// ```
pub fn function(param: Type) -> Result<ReturnType> {
    // implementation
}
```

**Example of good code:**

```rust
// Good
pub fn calculate_total_price(items: &[Item]) -> Result<f64, PriceError> {
    if items.is_empty() {
        return Err(PriceError::EmptyItemList);
    }
    Ok(items.iter().map(|item| item.price).sum())
}

// Avoid
pub fn calc(i: &[Item]) -> f64 {
    i.iter().map(|x| x.price).sum() // might panic on empty
}
```

### Python

We follow [PEP 8](https://pep8.org/) and [PEP 257](https://pep257.readthedocs.io/), enforced by `ruff`.

```bash
# Format code
ruff format .

# Lint code
ruff check .

# Type checking
mypy .
```

**Key guidelines:**

- Use `snake_case` for functions, methods, and variables
- Use `PascalCase` for classes
- Use `UPPER_CASE` for constants
- Maximum line length: 88 characters (ruff default)
- Use type hints everywhere
- Write docstrings for all public functions and classes (Google style)

**Documentation format:**

```python
def function(param: str) -> Result:
    """Brief description.

    Args:
        param: Description of parameter.

    Returns:
        Description of return value.

    Raises:
        ValueError: When param is invalid.
    """
    if not param:
        raise ValueError("param cannot be empty")
    return Result(param)
```

**Example of good code:**

```python
# Good
def calculate_total_price(items: list[Item]) -> float:
    """Calculate the total price of items.

    Args:
        items: List of items to calculate price for.

    Returns:
        The total price as a float.

    Raises:
        ValueError: If items list is empty.
    """
    if not items:
        raise ValueError("Items list cannot be empty")
    return sum(item.price for item in items)


# Avoid
def calc(i):
    return sum(x.price for x in i)
```

### TypeScript/React

We use TypeScript strict mode and follow [React best practices](https://react.dev/learn).

```bash
# Format code
npm run format
# or
npx prettier --write .

# Lint code
npm run lint
# or
npx eslint .

# Type check
npm run typecheck
# or
npx tsc --noEmit
```

**Key guidelines:**

- Use `camelCase` for functions, methods, and variables
- Use `PascalCase` for classes, interfaces, types, and components
- Use `UPPER_CASE` for constants
- Maximum line length: 100 characters
- Use TypeScript strict mode
- Prefer `const` over `let`, avoid `var`
- Use explicit return types for functions
- Prefer interfaces over type aliases for object shapes
- Document props with JSDoc comments

**Example of good code:**

```typescript
interface Props {
  /** The current value to display */
  value: string;
  /** Called when the value changes */
  onChange: (value: string) => void;
}

/**
 * A reusable input component with controlled value.
 */
export function InputField({ value, onChange }: Props): JSX.Element {
  return (
    <input
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

// Avoid
var InputField = (props) => <input value={props.value} onChange={props.onChange} />;
```

---

## Testing Requirements

All contributions must include appropriate tests. We maintain high test coverage to ensure reliability.

### General Testing Principles

1. **Write tests first** (TDD) when possible
2. **Test behavior, not implementation**
3. **Keep tests focused and independent**
4. **Use descriptive test names**
5. **Aim for >80% code coverage** but prioritize meaningful tests
6. **Include edge cases and error conditions**

### Running Tests

```bash
# All tests
make test

# Rust only
make rust-test

# Python only
make python-test

# Frontend only
make frontend-test
```

### Rust Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

**Example test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_total_price_returns_sum_of_item_prices() {
        let items = vec![
            Item { price: 10.0 },
            Item { price: 20.0 },
        ];

        let result = calculate_total_price(&items).unwrap();

        assert_eq!(result, 30.0);
    }

    #[test]
    fn calculate_total_price_returns_error_for_empty_list() {
        let items: Vec<Item> = vec![];

        let result = calculate_total_price(&items);

        assert!(result.is_err());
    }
}
```

### Python Testing

```bash
# Run all tests
pytest

# Run with coverage
pytest --cov=src --cov-report=html

# Run specific test file
pytest tests/test_module.py

# Run with verbose output
pytest -v
```

**Example test:**

```python
import pytest
from mymodule import calculate_total_price, Item


class TestCalculateTotalPrice:
    def test_returns_sum_of_item_prices(self):
        items = [Item(price=10.0), Item(price=20.0)]

        result = calculate_total_price(items)

        assert result == 30.0

    def test_raises_error_for_empty_list(self):
        with pytest.raises(ValueError, match="cannot be empty"):
            calculate_total_price([])

    def test_handles_single_item(self):
        items = [Item(price=15.0)]

        result = calculate_total_price(items)

        assert result == 15.0
```

### TypeScript Testing

```bash
# Run all tests
npm test

# Run with coverage
npm run test:coverage

# Run in watch mode
npm run test:watch

# Run specific test file
npm test -- path/to/test.spec.ts
```

**Example test:**

```typescript
import { describe, it, expect } from 'vitest'; // or jest
import { calculateTotalPrice } from './pricing';

describe('calculateTotalPrice', () => {
  it('returns the sum of item prices', () => {
    const items = [{ price: 10 }, { price: 20 }];

    const result = calculateTotalPrice(items);

    expect(result).toBe(30);
  });

  it('returns 0 for empty array', () => {
    const result = calculateTotalPrice([]);

    expect(result).toBe(0);
  });

  it('handles items with decimal prices', () => {
    const items = [{ price: 10.5 }, { price: 20.25 }];

    const result = calculateTotalPrice(items);

    expect(result).toBeCloseTo(30.75);
  });
});
```

### Coverage Requirements

| Area | Minimum Coverage |
|------|-----------------|
| New code | 80% |
| Critical paths | 95% |
| Bug fixes | 100% (for the fix) |

---

## Pull Request Process

### Before Submitting

1. **Ensure your code compiles** without warnings
2. **Run all checks**: `make lint test`
3. **Run linters and formatters** for your language
4. **Update documentation** if needed
5. **Add/update tests** for your changes
6. **Rebase on latest main** if needed

### Creating a Pull Request

1. **Push your branch** to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Open a Pull Request** against the `main` branch

3. **Fill out the PR template** completely:

```markdown
## Summary
Brief description of changes

## Changes
- Change 1
- Change 2

## Testing
How were these changes tested?

## Checklist
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] Lint passes
- [ ] All tests pass
```

4. **Request reviews** from appropriate maintainers

### PR Title Format

Use a clear, descriptive title following our commit message conventions:

```
feat(orchestrator): add circuit breaker for failure handling
fix(dag): resolve cycle detection edge case
docs(readme): update quick start guide
```

### Review Process

1. **Automated checks** must pass (CI/CD, linting, tests)
2. **At least one approval** from a maintainer required
3. **Address all review comments** or discuss alternatives
4. **Keep PRs focused** - one feature/fix per PR
5. **Squash commits** before merging

### After Merge

- Delete your feature branch
- Update related issues
- Celebrate your contribution!

---

## Commit Message Conventions

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification.

### Format

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Types

| Type | Description |
|------|-------------|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation only changes |
| `style` | Changes that don't affect code meaning (formatting) |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `perf` | Performance improvement |
| `test` | Adding or correcting tests |
| `build` | Changes to build system or dependencies |
| `ci` | Changes to CI configuration |
| `chore` | Other changes that don't modify src or test files |
| `revert` | Reverts a previous commit |

### Scope

The scope should be the name of the module/component affected (e.g., `orchestrator`, `dag`, `api`, `auth`, `parser`, `cli`, `ui`).

### Examples

```bash
# Feature with scope
feat(orchestrator): add circuit breaker for failure handling

# Bug fix
fix(dag): resolve cycle detection edge case

# Documentation
docs(readme): update quick start guide

# Breaking change (note the !)
feat(api)!: change response format for user endpoint

BREAKING CHANGE: The user endpoint now returns an object instead of an array.

# Commit with body
fix(cache): resolve memory leak in LRU cache

The cache was not properly evicting entries when the size limit
was reached, causing memory to grow unbounded over time.

Fixes #123
```

### Commit Message Guidelines

- Use the imperative mood: "add feature" not "added feature"
- Don't capitalize the first letter of the description
- No period at the end of the subject line
- Limit subject line to 72 characters
- Separate subject from body with a blank line
- Use the body to explain what and why, not how
- Reference issues and PRs in the footer

---

## Issue Templates

We provide templates to help you create effective issues.

### Bug Reports

Use the **Bug Report** template when you encounter unexpected behavior:

- **Title**: Clear, concise description of the bug
- **Environment**: OS, versions, configuration
- **Steps to Reproduce**: Detailed steps to reproduce the issue
- **Expected Behavior**: What should happen
- **Actual Behavior**: What actually happens
- **Screenshots/Logs**: If applicable

### Feature Requests

Use the **Feature Request** template for new functionality:

- **Problem Statement**: What problem does this solve?
- **Proposed Solution**: How should it work?
- **Alternatives Considered**: Other approaches you've thought about
- **Additional Context**: Mockups, examples, related issues

### Documentation Issues

Use the **Documentation** template for doc improvements:

- **Location**: Where is the issue?
- **Problem**: What's wrong or missing?
- **Suggested Improvement**: How can it be better?

### Templates Location

Issue templates are located in `.github/ISSUE_TEMPLATE/`. Feel free to suggest improvements to these templates!

---

## Architecture Decision Records (ADR)

We use Architecture Decision Records to document significant technical decisions.

### What Warrants an ADR?

- Changes to system architecture
- New technology adoption
- Significant API changes
- Security-related decisions
- Performance optimization strategies
- Breaking changes to existing functionality

### ADR Process

For significant architectural changes:

1. **Create an issue** describing the proposal
2. **Discuss with maintainers** to gather feedback
3. **Draft the ADR** with status "Proposed"
4. **Open a PR** for the ADR document
5. **Update status** to "Accepted" after approval
6. **Document** the decision in `docs/architecture/`

### ADR Format

ADRs are stored in `docs/architecture/` (or `docs/adr/`) using the following naming convention:

```
NNNN-short-title.md
```

Example: `0001-use-postgresql-for-persistence.md`

### ADR Template

```markdown
# ADR-NNNN: Title

## Status

[Proposed | Accepted | Deprecated | Superseded by ADR-XXXX]

## Context

What is the issue that we're seeing that is motivating this decision?

## Decision

What is the change that we're proposing and/or doing?

## Consequences

What becomes easier or more difficult because of this change?

### Positive

- Benefit 1
- Benefit 2

### Negative

- Drawback 1
- Drawback 2

### Neutral

- Side effect 1

## References

- Link to relevant discussions
- Link to related ADRs
- Link to implementation PRs
```

---

## Release Process

We follow [Semantic Versioning](https://semver.org/) (SemVer).

### Version Format

```
MAJOR.MINOR.PATCH
```

- **MAJOR**: Breaking changes
- **MINOR**: New features (backwards compatible)
- **PATCH**: Bug fixes (backwards compatible)

### Pre-release Versions

```
MAJOR.MINOR.PATCH-<pre-release>

Examples:
- 1.2.0-alpha.1
- 1.2.0-beta.2
- 1.2.0-rc.1
```

### Release Cycle

1. **Development**: All changes go to `main` branch
2. **Release Branch**: Created from `main` when ready (e.g., `release/v1.2.0`)
3. **Release Candidate**: Tagged as `v1.2.0-rc.1` for testing
4. **Final Release**: Tagged as `v1.2.0` after validation
5. **Hotfixes**: Cherry-picked to release branch if needed

### Release Checklist

- [ ] All tests passing on `main`
- [ ] CHANGELOG.md updated
- [ ] Version numbers updated in all relevant files
- [ ] Documentation updated
- [ ] Release notes drafted
- [ ] Security review completed (if applicable)
- [ ] Performance benchmarks run
- [ ] Migration guide written (for breaking changes)
- [ ] Stakeholders notified

### Changelog

We maintain a [CHANGELOG.md](CHANGELOG.md) following [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
## [Unreleased]

### Added
- New features

### Changed
- Changes in existing functionality

### Deprecated
- Features to be removed in future versions

### Removed
- Removed features

### Fixed
- Bug fixes

### Security
- Security fixes
```

---

## Getting Help

### Resources

- **Documentation**: Check out our [docs](docs/) folder
- **Discussions**: Use [GitHub Discussions](https://github.com/apex-swarm/apex/discussions) for questions
- **Discord**: Join our [Discord community](https://discord.gg/apex)

### Asking Questions

When asking for help:

1. **Search existing issues and discussions first**
2. **Provide context** about what you're trying to do
3. **Include relevant code snippets**, error messages, and logs
4. **Mention your environment** (OS, versions, etc.)
5. **Be specific** about what you've already tried

### Maintainer Response Time

| Type | Expected Response |
|------|-------------------|
| Issues | Triaged within 1 week |
| Pull Requests | Initial review within 2 weeks |
| Security issues | Addressed within 48 hours |

---

## Recognition

Contributors are recognized in several ways:

- Listed in [CONTRIBUTORS.md](CONTRIBUTORS.md)
- Mentioned in release notes for significant contributions
- Credited in the project's About page
- Featured in project showcases and blog posts

---

## Quick Reference

### Essential Commands

```bash
# Setup
make setup              # Install all dependencies
make docker-up          # Start infrastructure
make dev                # Start development servers

# Quality
make lint               # Run all linters
make test               # Run all tests
cargo fmt               # Format Rust code
ruff format .           # Format Python code
npm run lint            # Lint TypeScript/React

# Testing
make rust-test          # Run Rust tests
make python-test        # Run Python tests
make frontend-test      # Run frontend tests
```

### Checklist Before Submitting PR

- [ ] Branch follows naming convention
- [ ] Commits follow Conventional Commits
- [ ] Code is formatted (`cargo fmt`, `ruff format`, `npm run format`)
- [ ] Linters pass (`cargo clippy`, `ruff check`, `npm run lint`)
- [ ] All tests pass (`make test`)
- [ ] New tests added for new functionality
- [ ] Documentation updated if needed
- [ ] PR description is complete

---

Thank you for contributing to Apex! Your efforts help make this project better for everyone.

If you have suggestions for improving this guide, please open an issue or PR.

Happy coding!
