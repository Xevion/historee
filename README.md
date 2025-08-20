# historee

A fast Rust tool to analyze browser history and extract unique domains with visit counts.

- Analyze browser history databases
- Extract and normalize domain names from URLs
- Count visits per domain with parallel processing
- Custom domain pattern matching for normalization
- Privacy options with domain redaction
- Structured logging with tracing

## Installation

```bash
# Clone and build
git clone https://github.com/Xevion/historee.git
cd historee
cargo build --release

# Or install directly with cargo
cargo install --path .
```

## Usage

### Basic Analysis

```bash
# Analyze browser history
historee

# Show top 10 most visited domains
historee --top 10

# Show bottom 5 least visited domains
historee --bottom 5
```

### Advanced Options

```bash
# Use custom domain patterns
historee --patterns custom_patterns.txt

# Disable pattern-based normalization
historee --no-patterns

# Redact domain names for privacy
historee --redact

# Use specific number of worker threads
historee --workers 4

# Enable verbose logging
historee --verbose
```

### Initialize Default Patterns

```bash
# Create domain_patterns.txt with default patterns
historee --init
```

## Output Example

```
--- Browser History Analysis ---
Date range: February 9, 2025 to August 20, 2025 (191 days)
Total unique domains found: 4,132
Domains removed (no valid TLD): 976

Top 5 most visited domains:
- google.com: 1,234 visits
- github.com: 567 visits
- stackoverflow.com: 345 visits
- reddit.com: 234 visits
- youtube.com: 123 visits
```

## Supported Browsers

- **Vivaldi** (Windows, macOS, Linux)
- More browsers coming soon

## License

[Add your license here]
