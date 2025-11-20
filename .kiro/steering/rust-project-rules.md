---
inclusion: always
---

# Rust Project Rules

## Code Documentation

- **ALWAYS document code thoroughly** like existing code in the repository
- Add comments explaining:
  - What functions/methods do (purpose and behavior)
  - Complex logic and algorithms
  - Non-obvious parameter meanings
  - Return value descriptions
  - Error conditions and edge cases
- Use Rust doc comments (`///` for items, `//!` for modules)
- Remember: The maintainer is learning Rust and needs detailed explanations

## File Management

- **NEVER create markdown files** without explicit user consent
- **NEVER create .old or .bak files** for backups
- **ALWAYS edit files in place** using strReplace or similar tools
- If major refactoring is needed, discuss the approach first

## Code Quality Standards

- **ALL builds MUST pass** without errors or warnings
- **ALL clippy checks MUST pass** without warnings
- **ALL tests MUST pass** without errors or warnings
- Fix ALL issues, even if unrelated to current changes
- Run these checks before completing work:
  - `cargo build`
  - `cargo clippy -- -D warnings`
  - `cargo test`

## Testing Requirements

- **Tests MUST be in their own separate files**, never mixed in src/ implementation files
- Create dedicated test files (e.g., `command_tests.rs`, `app_type_tests.rs`)
- Test files can live alongside the code they test (e.g., in `src/utils/` or `src/detection/tests/`)
- Keep test code completely separate from implementation code

## Code Organization

- **Files exceeding 700 lines MUST be split** into smaller modules
- Break large files into logical, focused modules
- Use mod.rs to organize module structure
- Keep single responsibility principle in mind

## Command Usage

- **AVOID sed, awk, or similar text manipulation commands**
- These tools can easily break code syntax
- Use proper code editing tools (strReplace, fsWrite) instead
- Prefer Rust-aware tooling when available

## Additional Best Practices

- Follow existing code style and patterns in the repository
- Use meaningful variable and function names
- Keep functions focused and concise
- Handle errors properly with Result types
- Use idiomatic Rust patterns (iterators, pattern matching, etc.)
