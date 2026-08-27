# SparseIO General Architecture

## Overview

The general goal of SparseIO is to provide a flexible framework for building applications
managing large and complex data where access patterns are non-linear. This includes applications
in scientific computing, machine learning, and data analytics.

The goal is to make it easy for a developer to choose one or more upstream data
sources through a [`Reader`](./API.md#reader), a downstream cache through a
[`Writer`](./API.md#writer), and a key-value store through
[`Metadata`](./API.md#metadata).

## Table of Contents

- [Core Objects](./OBJECTS.md): An explanation of the core objects in our architecture, and
  the decisions behind their design.
  - [`SparseIO`](./OBJECTS.md#sparseio)
  - [`ReaderRegistry`](./OBJECTS.md#readerregistry)
  - [`Viewer`](./OBJECTS.md#viewer)
- [Trait API](./API.md): An explanation of the developer-facing API for using SparseIO with custom
  resources.
  - [`Reader`](./API.md#reader)
  - [`Writer`](./API.md#writer)
  - [`Metadata`](./API.md#metadata)
- [Testing](../testing/index.md): Testing documentation and feature-gated shared support.
  - [`testing` Feature](../testing/FEATURE.md)
  - [Trait Validation](../testing/VALIDATION.md)
- [CAS](./CAS.md): The content-addressable cache and chunk lifecycle.
- [Flow](./FLOW.md): How SparseIO reads, caches, and writes data.
- [Optimizations](./DECISIONS.md): Performance decisions that keep SparseIO efficient.
