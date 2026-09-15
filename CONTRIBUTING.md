# Contributing to MeraRAW

Thank you for your interest in contributing to MeraRAW! This document provides guidelines and information for contributors.

## Code of Conduct

We are committed to providing a welcoming and inclusive environment for all contributors. Please be respectful and professional in all interactions.

## Getting Started

### Prerequisites

- **Rust** (latest stable): Install from [rustup.rs](https://rustup.rs/)
- **Node.js** (v18+) and npm
- **Platform-specific requirements**:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#linux)
  - **Windows**: See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#windows)

### Building from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/meratech-llc/MeraRAW.git
   cd MeraRAW
   ```

2. Install JavaScript dependencies:
   ```bash
   npm install
   ```

3. Run in development mode:
   ```bash
   npx tauri dev
   ```

4. Run tests:
   ```bash
   cd src-tauri
   cargo test -p meratech-core
   ```

### Project Structure

- `src/` — Frontend UI (Svelte/TypeScript)
- `src-tauri/` — Tauri application shell
- `src-tauri/core/` — Core RAW processing engine (pure Rust, headless-testable)
- `src-tauri/resize/` — Image resizing utilities
- `vendor/` — Vendored engine crates (MERAWLER, ZERAWLER)
- `presets/` — Bundled color presets
- `docs/` — Additional documentation

## Development Workflow

### Running Tests

```bash
# Run Rust unit tests
cd src-tauri
cargo test -p meratech-core

# Run frontend tests
npm test

# Run self-test suite
MERATECH_DATA_DIR=/tmp/meratech-data \
  MERATECH_ASSISTANT_MOCK=1 \
  MERATECH_SELFTEST=1 \
  npx tauri dev
```

### Code Style

- **Rust**: Follow standard Rust formatting (`cargo fmt`)
- **TypeScript/JavaScript**: Follows project ESLint configuration
- **Commits**: Use clear, descriptive commit messages

### Headless Testing Tools

The project includes several headless visual check tools:

```bash
cd src-tauri
cargo run -p meratech-core --release --example decode_check -- <raw-file>
cargo run -p meratech-core --release --example render_check -- <raw-file>
cargo run -p meratech-core --release --example mask_check -- <raw-file>
cargo run -p meratech-core --release --example export_check -- <raw-file>
```

## How to Contribute

### Reporting Issues

- Use the GitHub issue tracker
- Include:
  - Operating system and version
  - MeraRAW version
  - Steps to reproduce
  - Expected vs. actual behavior
  - Sample files if applicable (for RAW processing issues)
  - Relevant logs or screenshots

### Submitting Pull Requests

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/your-feature-name`
3. Make your changes
4. Add tests if applicable
5. Ensure tests pass: `cargo test -p meratech-core && npm test`
6. Commit with clear messages
7. Push to your fork
8. Open a pull request

### Pull Request Guidelines

- Keep changes focused and atomic
- Include tests for new functionality
- Update documentation as needed
- Ensure CI passes
- Respond to review feedback

## Architecture Overview

MeraRAW is built on four core contracts:

1. **EditDoc** (`core/src/doc.rs`) — Versioned state document with sidecar persistence
2. **Param registry** (`core/src/registry.rs`) — Single source of truth for parameters
3. **Ops + guard-wall** (`core/src/ops.rs`) — The only write path to the document
4. **Working texture** — Linear Rec.2020 scene-referred RGBA16F pipeline

The render graph is a cache-the-chain design that only re-renders from the first dirty stage.

See the [README](./README.md) for more architectural details.

## Areas for Contribution

Some areas where contributions are particularly welcome:

- **Camera support**: Additional RAW format testing and compatibility
- **Color profiles**: DCP camera profiles for additional camera models
- **Presets**: Creative color grading presets
- **Performance**: GPU optimization, memory efficiency
- **Testing**: Additional test coverage, especially for edge cases
- **Documentation**: Improved guides, tutorials, API documentation
- **Accessibility**: UI improvements for accessibility
- **Localization**: Translations (when i18n infrastructure is added)

## Questions?

- Open a GitHub Discussion for general questions
- Check existing documentation in the `docs/` directory
- Review closed issues for similar questions

## License

By contributing to MeraRAW, you agree that your contributions will be licensed under the Apache License 2.0. See [LICENSE](./LICENSE) for details.
