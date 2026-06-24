//! The command registry: built-in and plugin commands are registered alike.

use crate::command::Command;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct Registry {
    commands: HashMap<String, Arc<dyn Command>>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Registry::default()
    }

    pub fn register(&mut self, command: impl Command + 'static) {
        let name = command.signature().name;
        self.commands.insert(name, Arc::new(command));
    }

    pub fn register_arc(&mut self, command: Arc<dyn Command>) {
        let name = command.signature().name;
        self.commands.insert(name, command);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Command>> {
        self.commands.get(name).cloned()
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut n: Vec<String> = self.commands.keys().cloned().collect();
        n.sort();
        n
    }
}
