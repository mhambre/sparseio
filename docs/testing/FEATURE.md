# `testing` Feature

Shared testing utilities, implementations, and hooks are defined in the library code
and hidden from the default public API behind the `testing` feature. This allows test
support to be reused and deduplicated across unit, integration, and
[trait-validation](./VALIDATION.md) tests while keeping the public API and production
code slim.
