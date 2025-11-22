// Config module tests - organized by functionality
//
// This module contains focused test suites for different aspects of config handling:
// - resolution_tests: App resolution and lookup functionality
// - dependency_tests: Dependency chain resolution and ordering
// - validation_tests: Error handling, circular dependencies, and validation

pub mod dependency_tests;
pub mod resolution_tests;
pub mod validation_tests;