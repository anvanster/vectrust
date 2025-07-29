# Contributing to Vectrust

We welcome contributions to Vectrust! This guide will help you get started with contributing to the project.

## 🚀 Getting Started

### Prerequisites

- Rust 1.70+ with Cargo
- Node.js 14+ (for Node.js bindings)
- Git

### Development Setup

1. **Fork and Clone**
   ```bash
   git clone https://github.com/your-username/vectrust.git
   cd vectrust
   ```

2. **Build the Project**
   ```bash
   cargo build --release
   ```

3. **Run Tests**
   ```bash
   cargo test
   ```

4. **Run Benchmarks**
   ```bash
   cd benchmarks/rust
   cargo run --release --bin benchmark_runner -- --simple
   ```

## 🛠️ Development Workflow

### Code Style

- Follow standard Rust formatting: `cargo fmt`
- Ensure code passes linting: `cargo clippy`
- Write comprehensive tests for new features
- Document public APIs with doc comments

### Testing

- **Unit Tests**: Add tests in the same file as your code using `#[cfg(test)]`
- **Integration Tests**: Add tests in `tests/` directory
- **Benchmarks**: Update benchmarks if you change performance-critical code

### Commit Messages

Use clear, descriptive commit messages:
```
feat: add HNSW indexing support
fix: resolve deadlock in optimized storage
docs: update README with new performance metrics
```

## 📝 Types of Contributions

### 🐛 Bug Reports

When filing bug reports, please include:
- **Description**: Clear description of the issue
- **Reproduction Steps**: Minimal code to reproduce the bug
- **Environment**: OS, Rust version, Vectrust version
- **Expected vs Actual**: What you expected vs what happened

### ✨ Feature Requests

For new features:
- **Use Case**: Describe the problem you're trying to solve
- **Proposed Solution**: Your suggested approach
- **Alternatives**: Other solutions you've considered
- **Breaking Changes**: Whether this would break existing APIs

### 🔧 Code Contributions

1. **Create a Feature Branch**
   ```bash
   git checkout -b feature/amazing-feature
   ```

2. **Make Your Changes**
   - Write code following our style guidelines
   - Add tests for new functionality
   - Update documentation if needed

3. **Test Your Changes**
   ```bash
   cargo test
   cargo clippy
   cargo fmt --check
   ```

4. **Run Benchmarks** (if performance-related)
   ```bash
   cd benchmarks/rust
   cargo run --release --bin benchmark_runner
   ```

5. **Commit and Push**
   ```bash
   git add .
   git commit -m "feat: add amazing feature"
   git push origin feature/amazing-feature
   ```

6. **Create Pull Request**
   - Provide clear description of changes
   - Reference any related issues
   - Include benchmark results if applicable

## 🏗️ Project Structure

```
vectrust-rust/
├── crates/
│   ├── vectrust-core/      # Core types and traits
│   ├── vectrust-storage/   # Storage backends
│   ├── vectrust-index/     # Indexing algorithms  
│   ├── vectrust-query/     # Query processing
│   ├── vectrust/           # Main library
│   ├── vectrust-cli/       # Command-line tool
│   └── vectrust-node/      # Node.js bindings
├── benchmarks/             # Performance benchmarks
├── examples/               # Usage examples
└── tests/                  # Integration tests
```

## 🎯 Areas for Contribution

### High Priority
- **Performance Optimizations**: Vector operations, storage efficiency
- **New Indexing Algorithms**: LSH, Product Quantization
- **Language Bindings**: Python, Go, Java support
- **Documentation**: API docs, tutorials, examples

### Medium Priority
- **Storage Backends**: S3, GCS integration
- **Monitoring**: Metrics and health checks
- **Advanced Features**: Hybrid search, streaming updates
- **Platform Support**: Windows, macOS compatibility

### Low Priority
- **UI Tools**: Web interface, visualization
- **Cloud Integration**: Kubernetes operators
- **Advanced Analytics**: Query optimization insights

## 🔍 Code Review Process

1. **Automated Checks**: CI/CD runs tests, linting, and benchmarks
2. **Peer Review**: Maintainers review code for quality and design
3. **Performance Review**: Benchmark results reviewed for regressions
4. **Documentation Review**: Ensure changes are properly documented

## 🤝 Community Guidelines

- **Be Respectful**: Treat all contributors with respect
- **Be Patient**: Reviews and responses may take time
- **Be Constructive**: Provide helpful feedback and suggestions
- **Ask Questions**: Don't hesitate to ask for clarification

## 📚 Resources

- **Documentation**: [docs.rs/vectrust](https://docs.rs/vectrust)
- **API Reference**: See `cargo doc --open`
- **Benchmarks**: Check `benchmarks/` directory
- **Examples**: See `examples/` directory

## 🆘 Getting Help

- **GitHub Issues**: For bugs and feature requests
- **GitHub Discussions**: For questions and community chat
- **Documentation**: For API usage and examples

## 🏆 Recognition

Contributors are recognized in:
- **README.md**: Major contributors listed
- **Changelog**: All contributors credited in releases
- **GitHub**: Contribution graphs and statistics

Thank you for contributing to Vectrust! 🦀❤️