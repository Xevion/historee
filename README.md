# historee

A fast Rust tool to analyze browser history and extract unique domains with visit counts.

- Analyze multiple browser history databases (Chrome, Edge, Firefox, Vivaldi, Zen)
- Extract and normalize domain names from URLs with parallel processing
- Custom domain pattern matching for normalization
- Privacy options with domain redaction
- Structured logging with tracing
- Cross-platform support with platform-specific optimizations

## Installation

```bash
# Clone and build
git clone https://github.com/Xevion/historee.git
cd historee
cargo build --release

# Or install directly with cargo
cargo install --path .

# Or use the Justfile
just install
```

## Usage

### Basic Analysis

```bash
# Analyze default browser (Vivaldi)
historee

# Analyze specific browser
historee --browser chrome
historee --browser firefox
historee --browser edge
historee --browser vivaldi
historee --browser zen

# Analyze all supported browsers
historee --all-browsers

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

# Specify custom temporary file path
historee --temp-path /tmp/custom_history.db
```

### Initialize Default Patterns

```bash
# Create domain_patterns.txt with default patterns
historee --init
```

## Output Example

```
--- Vivaldi History Analysis ---
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

- **Chrome** (Windows, macOS, Linux)
- **Edge** (Windows, macOS, Linux)
- **Firefox** (Windows, macOS, Linux)
- **Vivaldi** (Windows, macOS, Linux)
- **Zen** (Windows, macOS, Linux)
