# Contributing

All contributions are welcome! Whether it's bug fixes, new features, documentation improvements, or bug reports - we appreciate your help in making webrockets better.

## Performance

This project cares a lot about performance. The core server is written in Rust specifically for this reason. If you're touching code in performance-critical paths, please keep this in mind and consider the performance implications of your changes.

## Development Setup

### Prerequisites

- Python 3.9+
- Rust toolchain (for building the native extension)
- [uv](https://docs.astral.sh/uv/) (Python package manager)

### Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/ploMP4/webrockets.git
   cd webrockets
   ```

2. **Install dependencies:**
   ```bash
   uv sync --dev
   ```

3. **Activate the virtual environment:**
   ```bash
   source .venv/bin/activate
   ```

4. **Build the Rust extension (development mode):**
   ```bash
   uvx maturin develop
   ```

   For a release build with optimizations:
   ```bash
   uvx maturin build --release
   ```

## Running Tests

Before submitting a pull request, make sure all tests pass and code style is correct:

```bash
# Run tests
pytest

# Run linter
ruff check .

# Run formatter check
ruff format --check .
```

## Commit Messages

[Conventional commits](https://www.conventionalcommits.org/) are appreciated but not required. If you use them, here are some common prefixes:

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation changes
- `refactor:` - Code refactoring
- `test:` - Adding or updating tests
- `perf:` - Performance improvements

## Before You Start

For **big features**, **breaking changes**, or if you're **unsure** about the approach, please [open an issue](https://github.com/ploMP4/webrockets/issues) first so it can be discussed. This helps ensure your contribution aligns with the project's direction and saves you time if changes are needed.

For smaller bug fixes and improvements, feel free to submit a pull request directly.

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b my-feature`)
3. Make your changes
4. Run tests and linting
5. Commit your changes
6. Push to your fork
7. Open a pull request

Thank you for contributing!
