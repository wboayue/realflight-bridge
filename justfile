# Builds the project
build:
    cargo build

# Runs tests
test:
    cargo test

# Runs benchmarks
bench:
    cargo bench --features bench-internals

# Runs benchmarks
bench2:
    cargo run --example parse --features bench-internals

# Generate and save coverage report using tarpaulin
cover:
    cargo tarpaulin -o html
    echo "coverage report saved to tarpaulin-report.html"

# Tags repo with specified version
tag VERSION:
    echo "Tagging repo with version {{VERSION}}"
    git tag {{VERSION}} -m "Version {{VERSION}}"
    git push origin {{VERSION}}

# Lists all available versions
versions:
    @git tag
