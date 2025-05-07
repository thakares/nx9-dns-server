# Contributing to nx9-dns-server

Thank you for considering contributing to nx9-dns-server! This document provides guidelines and instructions to help you contribute effectively to this project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
  - [Project Setup](#project-setup)
  - [Development Environment](#development-environment)
- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Code Contributions](#code-contributions)
- [Pull Request Process](#pull-request-process)
- [Style Guidelines](#style-guidelines)
  - [Rust Code Style](#rust-code-style)
  - [Commit Messages](#commit-messages)
  - [Documentation](#documentation)
- [Priority Areas](#priority-areas)
- [Community](#community)
- [License](#license)

## Code of Conduct

By participating in this project, you are expected to uphold our [Code of Conduct](CODE_OF_CONDUCT.md). Please report unacceptable behavior to [project maintainers](mailto:maintainer@example.com).

## Getting Started

### Project Setup

1. **Fork the repository** on GitHub
2. **Clone your fork**:
   ```bash
   git clone https://github.com/your-username/nx9-dns-server.git
   cd nx9-dns-server
   ```
3. **Add the upstream remote**:
   ```bash
   git remote add upstream https://github.com/thakares/nx9-dns-server.git
   ```
4. **Create a branch** for your work:
   ```bash
   git checkout -b feature/your-feature-name
   ```

### Development Environment

#### Requirements
- Rust (stable, 1.70+)
- SQLite 3.x
- Cargo and standard Rust toolchain

#### Setup
1. **Install dependencies**:
   ```bash
   # For Debian/Ubuntu
   sudo apt-get install build-essential pkg-config libsqlite3-dev
   
   # For Fedora/RHEL
   sudo dnf install gcc sqlite-devel pkgconfig
   
   # For macOS with Homebrew
   brew install sqlite
   ```

2. **Compile and run the project**:
   ```bash
   cargo build
   cargo run
   ```

3. **Run tests**:
   ```bash
   cargo test
   ```

## How to Contribute

### Reporting Bugs

Before submitting a bug report:
- Check the [issue tracker](https://github.com/thakares/nx9-dns-server/issues) to see if the issue has already been reported
- Make sure you're using the latest version of the software
- Perform a quick search to see if the problem has already been addressed

When submitting a bug report:
1. Use the bug report template provided
2. Include a clear and descriptive title
3. Describe the exact steps to reproduce the issue
4. Provide specific examples to demonstrate the steps
5. Describe the behavior you observed and what you expected to see
6. Include relevant logs, screenshots, or other materials
7. Mention your environment (OS, Rust version, etc.)

### Suggesting Enhancements

Enhancement suggestions are tracked as GitHub issues. When creating an enhancement suggestion:
1. Use the feature request template provided
2. Include a clear and descriptive title
3. Provide a detailed description of the proposed functionality
4. Explain why this enhancement would be useful to most users
5. List any alternatives you've considered
6. Include any mockups or examples if applicable

### Code Contributions

We're actively seeking contributions in these areas:

1. **Web UI Development**
   - Frontend components and integration with backend
   - UI/UX design for DNS management

2. **API Service**
   - RESTful API implementation
   - Authentication and permission handling
   - Request validation

3. **User Management**
   - Authentication systems
   - Role-based access control
   - User onboarding flows

4. **DNSSEC Improvements**
   - Key rotation automation
   - Signature verification tools
   - DNSSEC validation utilities

5. **Core DNS Improvements**
   - Performance optimizations
   - Additional record type support
   - Protocol extensions

6. **Documentation and Testing**
   - Improving guides and examples
   - Unit and integration tests
   - Benchmarking tools

## Pull Request Process

1. **Update your fork** with the latest from upstream:
   ```bash
   git fetch upstream
   git merge upstream/main
   ```

2. **Implement your changes** and commit them to your feature branch

3. **Run the test suite** to ensure your changes don't break existing functionality:
   ```bash
   cargo test
   ```

4. **Add or update tests** as needed for your new functionality

5. **Update documentation** including README.md if needed

6. **Submit a pull request** to the main repository:
   - Fill out the PR template completely
   - Reference any related issues (e.g., "Fixes #123")
   - Include a clear description of the changes and their motivation
   - Add screenshots or terminal output if relevant

7. **Code review process**:
   - Maintainers will review your PR
   - Address any requested changes or feedback
   - Once approved, maintainers will merge your PR

## Style Guidelines

### Rust Code Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` to format your code:
  ```bash
  cargo fmt
  ```
- Use `clippy` to catch common mistakes and non-idiomatic code:
  ```bash
  cargo clippy
  ```
- Follow the existing project style for consistency
- Use meaningful variable and function names
- Include comments for complex sections of code
- Write comprehensive documentation for public API functions

### Commit Messages

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less
- Reference issues and pull requests after the first line
- Consider using a structured format:
  ```
  [Component] Short summary (up to 72 chars)
  
  More detailed explanation, if necessary. Wrap lines at around 72
  characters. Explain the problem this commit is solving. Focus on why
  you are making this change as opposed to how.
  
  Fixes #123
  ```

### Documentation

- Use proper grammatical sentences with punctuation
- Keep documentation up-to-date with code changes
- Include examples where appropriate
- Document all public API functions, structs, and traits
- Use Markdown formatting in doc comments and documentation files

## Priority Areas

We are particularly interested in contributions in these areas:

1. **Web UI Development**:
   - Creating a responsive, user-friendly interface for DNS management
   - Implementing dashboard components for monitoring DNS health
   - Building forms for record management with validation

2. **API Service**:
   - Implementing RESTful endpoints for DNS record CRUD operations
   - Adding authentication and authorization mechanisms
   - Developing batch operations for efficient record updates

3. **User Management**:
   - Building a role-based access control system
   - Implementing secure authentication flows
   - Creating administrative tools for user management

4. **Documentation**:
   - Improving guides and examples
   - Creating API documentation
   - Adding diagrams and architecture documentation

5. **Testing**:
   - Unit tests for core components
   - Integration tests for end-to-end validation
   - Building automated CI pipelines

## Community

- Join our [Discord server](https://discord.com/channels/1179651660184817714/1369586647393370253) for discussions
- Follow the project on [Twitter](https://x.com/thakares)
- Subscribe to our [mailing list](https://example.com/mailing-list) for updates

## License

By contributing to nx9-dns-server, you agree that your contributions will be licensed under the project's [GNU General Public License v3.0 (GPLv3)](LICENSE).

---

Thank you for your interest in improving nx9-dns-server! We appreciate your time and effort in contributing to this project.
