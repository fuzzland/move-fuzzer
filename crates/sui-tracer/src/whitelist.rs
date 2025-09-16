use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct WhitelistChecker {
    pub ignored_modules: HashSet<String>,
    pub ignored_functions: HashSet<String>,
}

impl Default for WhitelistChecker {
    fn default() -> Self {
        Self {
            ignored_modules: HashSet::new(),
            ignored_functions: HashSet::new(),
        }
    }
}

impl WhitelistChecker {
    /// Check if the specified module and function should be ignored
    pub fn should_ignore(&self, module: &str, function: &str) -> bool {
        if self.ignored_modules.contains(module) {
            return true;
        }

        if self.ignored_functions.contains(function) {
            return true;
        }

        false
    }
}
