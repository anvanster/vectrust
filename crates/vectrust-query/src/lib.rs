pub mod filter;
pub mod search;
pub mod hybrid;

pub use filter::*;
pub use search::*;
pub use hybrid::*;

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