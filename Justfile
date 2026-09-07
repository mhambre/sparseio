set shell := ["bash", "-cu"]

core_features := "testing,memory-metadata,disk-writer,opendal-reader-fs,validators"
component_benchmark_features := "memory-metadata,disk-writer,opendal-reader-fs"
api_benchmark_features := "memory-metadata,disk-writer,opendal-reader-hf"

default: check

# Build the library with the supported core feature composition.
build:
    cargo build --features {{core_features}}

# Check all core library and test targets.
check:
    cargo check --all-targets --features {{core_features}}

# Format project sources.
format:
    cargo +nightly fmt --all

alias fmt := format

# Run the complete unit, integration, feature, and documentation test suite.
test:
    cargo nextest run --all-targets --features {{core_features}}
    cargo test --doc --features {{core_features}}

# Enforce formatting and Clippy policy.
lint:
    cargo +nightly fmt --all -- --check
    cargo clippy --all-targets --features {{core_features}} -- -D warnings
    cargo clippy --bench api_workloads --features {{api_benchmark_features}} -- -D warnings

# Build public API documentation with documentation warnings denied.
docs:
    RUSTDOCFLAGS='-D warnings -D missing-docs' cargo doc --no-deps --features {{core_features}}

# Render both animations or one named animation.
animations name="all":
    #!/usr/bin/env bash
    set -euo pipefail
    case {{quote(name)}} in
        all) selected=(general-read cas) ;;
        general-read|cas) selected=({{quote(name)}}) ;;
        *) echo "Unknown animation. Choose all, general-read, or cas." >&2; exit 1 ;;
    esac
    for animation in "${selected[@]}"; do
        bash "scripts/animations/$animation/render.sh"
    done

# Enforce complete production implementation coverage.
coverage:
    cargo llvm-cov clean --workspace
    cargo llvm-cov --workspace --features {{core_features}} --no-report
    cargo llvm-cov report --ignore-filename-regex 'src/utils/testing/' --fail-uncovered-lines 0 --fail-uncovered-functions 0 --fail-uncovered-regions 0

# Run system-level component benchmarks, optionally filtered by name.
bench-components filter="":
    #!/usr/bin/env bash
    set -eu
    if [[ -z {{quote(filter)}} ]]; then
        exec cargo bench --benches --features {{component_benchmark_features}}
    fi
    exec cargo bench --benches --features {{component_benchmark_features}} -- {{quote(filter)}}

# Run controlled API workloads with latency percentiles and upstream counters.
bench-api profile="lan" requests="1000" fanout="32" format="table":
    cargo bench --bench api_workloads --features {{api_benchmark_features}} -- --profile {{quote(profile)}} --requests {{quote(requests)}} --fanout {{quote(fanout)}} --format {{quote(format)}}

# Run non-reproducible API reads against an explicitly supplied Hugging Face URI.
bench-api-live uri requests="100" format="table":
    cargo bench --bench api_workloads --features {{api_benchmark_features}} -- --live-uri {{quote(uri)}} --requests {{quote(requests)}} --format {{quote(format)}}

# Execute every benchmark path with minimal samples.
bench-smoke:
    cargo bench --benches --features {{component_benchmark_features}} -- --test
    cargo bench --bench api_workloads --features {{api_benchmark_features}} -- --test

# Run component benchmarks and the default controlled API workload.
bench:
    just bench-components
    just bench-api

# Run the complete local acceptance workflow.
ci: lint test docs coverage
