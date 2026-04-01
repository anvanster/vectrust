// Copyright 2024-2026 Andrey Vasilevsky <anvanster@gmail.com>
// SPDX-License-Identifier: Apache-2.0

pub mod filter;
pub mod hybrid;
pub mod search;

pub use filter::*;
pub use hybrid::*;
pub use search::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_module_exports() {
        // Test that all modules are properly exported
        // This is a simple smoke test to ensure compilation works
        assert!(true);
    }
}
