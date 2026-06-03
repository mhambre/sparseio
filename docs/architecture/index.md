# SparseIO General Architecture

## Overview

The general goal of SparseIO is to provide a flexible framework for building applications
managing large and complex data where access patterns are non-linear. This includes applications
in scientific computing, machine learning, and data analytics.

The goal is to make it as easy as possible for a developer to choose one-or-more upstream data
sources to read from ([Reader](./API.md#Reader)), a downstream location to write cache to ([Writer](./API.md#Writer)),
and a metadata management KV store to keep track of the data ([Metadata](./API.md#Metadata)).

## Table of Contents

- [Core Objects](./OBJECTS.md): An explanation of the core objects in our architecture, and
  the decisions behind their design.
  - [SparseIO Object](./OBJECTS.md#SparseIOObject)
  - [ReaderRegistry](./OBJECTS.md#ReaderRegistry)
  - [Viewer](./OBJECTS.md#Viewer)
- [Trait API](./API.md): An explanation of the Developer-facing API for leveraging SparseIO with custom
  resources.
  - [Reader](./API.md#Reader)
  - [Writer](./API.md#Writer)
  - [Metadata](./API.md#Metadata)
