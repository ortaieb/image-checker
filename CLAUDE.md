# IMAGE CHECKER - Implementation Guide

## 🔄 Project Awareness & Context
- **Always read `PLANNING.md`** at the start of a new conversation to understand the project's architecture, goals, style, and constraints.
- **Check `TASK.md`** before starting a new task. If the task isn’t listed, add it with a brief description and today's date.
- **Use consistent naming conventions, file structure, and architecture patterns** as described in `PLANNING.md`.

## 🧱 Code Structure & Modularity
- **Never create a file longer than 500 lines of code.** If a file approaches this limit, refactor by splitting it into modules or helper files.

## 🧪 Testing & Reliability

### Test Organization

- Place unit tests in the same file as the code they test using #[cfg(test)] modules
- Use integration tests in the tests/ directory for testing public APIs
- Keep test functions focused on a single behavior or edge case
- Use descriptive test names that explain what is being tested and expected outcome

### Testing Best Practices

- Always test error paths, not just happy paths
- Use #[should_panic(expected = "error message")] for testing expected panics
- Leverage property-based testing with proptest or quickcheck for complex invariants
- Test both owned and borrowed versions of your APIs when applicable
- Use cargo test -- --nocapture to see println! output during test debugging

### Async Testing

- Use #[tokio::test] or #[async_std::test] for async test functions
- Be aware of runtime differences between test and production environments
- Test timeout scenarios and cancellation safety for async code
- Avoid blocking operations in async tests that could cause deadlocks

### Error Handling in Tests

- Use Result<(), Box<dyn Error>> as test return type for using ? operator
- Prefer explicit error handling over .unwrap() in test setup code
- Create test-specific error types when needed for better diagnostics

### Mocking & Test Doubles

- Use trait objects for dependency injection to enable mocking
- Consider mockall or similar crates for complex mocking scenarios
- Be cautious with global state in tests - use std::sync::Once or serial_test when necessary
- Reset any modified global state in test teardown

### Benchmarking

- Use criterion for micro-benchmarks instead of built-in bench feature
- Warm up the CPU before benchmarking to get consistent results
- Test with release mode optimizations: cargo test --release
- Be aware of compiler optimizations that might invalidate benchmarks

### Safety Testing

- Use miri to detect undefined behavior in unsafe code
- Test unsafe code with various memory allocators to catch heap issues
- Verify that unsafe invariants are maintained across API boundaries
- Document safety requirements in tests for unsafe functions

## ✅ Task Completion
- **Mark completed tasks in `TASK.md`** immediately after finishing them.
- Add new sub-tasks or TODOs discovered during development to `TASK.md` under a “Discovered During Work” section.

## 📎 Style & Conventions

### Naming Conventions

- Use snake_case for functions, variables, modules, and crates
- Use CamelCase for types (structs, enums, traits)
- Use SCREAMING_SNAKE_CASE for constants and statics
- Prefix unused variables with underscore: _unused_var
- Use descriptive names over abbreviations (prefer calculate_total over calc_tot)

### Error Handling Patterns

- Create custom error types implementing std::error::Error
- Use thiserror for ergonomic error definitions
- Avoid unwrap() in production code - use expect() with meaningful messages
- Return Result<T, E> from fallible functions, not Option<T>
- Use the ? operator for error propagation instead of manual matching

### Memory & Performance

- Prefer borrowing (&T) over cloning when possible
- Use Cow<'a, T> for APIs that might or might not need ownership
- Be explicit about allocations - prefer Vec::with_capacity when size is known
- Avoid unnecessary collect() calls - use iterators directly when possible
- Use Box<dyn Trait> sparingly - prefer generics for zero-cost abstractions

### API Design

- Make invalid states unrepresentable using the type system
- Use builder pattern for complex struct construction
- Provide both consuming and borrowing versions of methods when appropriate
- Use #[must_use] on functions returning important values
- Design APIs to be hard to misuse - prefer type safety over runtime checks

### Documentation

- Write doc comments for all public items
- Include usage examples in doc comments using  ```rust blocks
- Document panics, errors, and safety requirements
- Use #![warn(missing_docs)] to enforce documentation
- Link related items using [item] syntax in docs

### Module Organization

- Keep modules small and focused on a single responsibility
- Use pub(crate) for crate-internal APIs instead of pub
- Re-export commonly used items at crate root for convenience
- Avoid deep module nesting - prefer flatter hierarchies
- Use mod.rs files sparingly in Rust 2018+ edition

### Lifetime & Borrowing

- Name lifetimes descriptively when multiple are involved
- Prefer lifetime elision where possible
- Avoid self-referential structs - they're usually a design smell
- Use Pin<T> and PhantomPinned correctly for self-referential needs
- Understand variance implications when using lifetimes in type parameters

### Concurrent Code

- Prefer message passing over shared state
- Use Arc<Mutex<T>> or Arc<RwLock<T>> for shared state when necessary
- Be aware of Send and Sync bounds for thread safety
- Avoid holding locks across await points in async code
- Use crossbeam channels over std::sync::mpsc for better performance

### Common Pitfalls to Avoid

- Don't implement Copy for types that own resources
- Avoid std::mem::forget unless absolutely necessary
- Be cautious with transmute - prefer safe alternatives
- Don't rely on Drop for critical cleanup in the presence of panics
- Avoid integer overflow in release mode - use checked arithmetic when needed
- Be aware of iterator invalidation rules when mutating collections

## 📚 Documentation & Explainability
- **Update `README.md`** when new features are added, dependencies change, or setup steps are modified.
- **Comment non-obvious code** and ensure everything is understandable to a mid-level developer.
- When writing complex logic, **add an inline `# Reason:` comment** explaining the why, not just the what.

## 🧠 AI Behavior Rules
- **Never assume missing context. Ask questions if uncertain.**
- **Never hallucinate libraries or functions** – only use known, verified Python packages.
- **Always confirm file paths and module names** exist before referencing them in code or tests.
- **Never delete or overwrite existing code** unless explicitly instructed to or if part of a task from `TASK.md`.