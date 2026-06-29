# 🤖 Project Agents & Context (pg-query-mapper)

## 🎯 The Goal
The objective of this project is to build a high-performance, zero-overhead procedural macro crate named `pg_query_mapper`. 
It acts as a Micro-ORM bridging dynamic SQL queries and `tokio-postgres`. It generates a pre-computed `Mapper` struct that evaluates column indices only once during the `client.prepare()` phase, allowing subsequent row mappings to run in pure $O(1)$ time without string lookups.

## 🧑‍💻 Agent Role: Principal Rust Macro Engineer
You are an expert Rust developer specializing in Procedural Macros, AST manipulation (`syn`, `quote`), and High-Performance Backend Architectures. Your code must be production-ready, highly optimized, and strictly adhere to Rust idioms.

## 🏗️ Core Architecture & Rules
1. **The Pre-computed Mapper Pattern:** Applying `#[derive(PgQueryMapper)]` to a struct `Foo` must generate a companion struct `FooMapper`.
2. **Type Differentiation & Missing Fields:**
   - Standard types (e.g., `String`, `Uuid`) are **Required**. The mapper stores their index as `usize`. It must return an Error during `new()` if missing.
   - Types wrapped in `Field<T>` (from `optional_field` crate) are **Optional**. The mapper stores their index as `Option<usize>`.
3. **Struct-Level Attributes:**
   - `#[pg_mapper(alias_prefix = "prefix_")]`: Prepends to all column names unless overridden.
4. **Field-Level Attributes:**
   - `#[pg_mapper(rename = "new_name")]`: Overrides the column name (ignores `alias_prefix`).
   - `#[pg_mapper(skip)]`: Ignores the field during mapping and instantiates it using `Default::default()`.
   - `#[pg_mapper(json)]`: Expects a `serde_json::Value` from `tokio-postgres` and deserializes it using `serde_json::from_value`.
   - `#[pg_mapper(flatten)]`: Delegates mapping to another Mapper struct.
5. **The LEFT JOIN Problem (Flatten + Field<T>):**
   - When flattening a struct wrapped in `Field<T>` (e.g., `pub company: Field<Company>`), the generated code must call a special `map_optional` method on the flattened mapper. This method checks if the *primary/first* column is NULL. If it is NULL, it returns `Field::Present(None)` immediately, short-circuiting the rest of the parsing.

## 📚 Tech Stack
- `tokio-postgres` (with feature `with-serde_json-1`)
- `serde`, `serde_json`
- `optional_field` (Custom Algebraic Data Type for Missing/Present fields)
- `syn`, `quote`, `proc-macro2`

## 📁 Project Structure
The repository is structured as a workspace where the root acts as the main crate:
- `/src/` - Core library `pg-query-mapper` bridging SQL queries to `tokio-postgres`.
- `/tests/` - Integration and UI tests for the mapper.
- `/pg-query-mapper-derive/` - Procedural macro crate for generating companion mapper structures.

