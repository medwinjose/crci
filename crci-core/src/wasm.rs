// Migration path: replace PriorityScorer::execute() with
// wasmtime::Engine::new() + Module::from_binary() when the
// wasmtime crate is available. The WasmModule trait is the
// stable interface — callers don't change.

use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum WasmError {
    MemoryLimitExceeded { limit: usize, requested: usize },
    ExecutionTimeout { limit_ms: u64 },
    InvalidInput(String),
    ModuleNotFound(String),
}

pub trait WasmModule: Send + Sync {
    fn name(&self) -> &str;
    fn execute(&self, input: &[u8]) -> Result<Vec<u8>, WasmError>;
    fn memory_limit_bytes(&self) -> usize;
}

#[allow(dead_code)]
pub struct WasmRuntime {
    modules: HashMap<String, Box<dyn WasmModule>>,
    memory_limit: usize,
    timeout_ms: u64,
}

impl WasmRuntime {
    pub fn new(memory_limit_bytes: usize, timeout_ms: u64) -> Self {
        Self {
            modules: HashMap::new(),
            memory_limit: memory_limit_bytes,
            timeout_ms,
        }
    }

    pub fn register(&mut self, module: Box<dyn WasmModule>) {
        self.modules.insert(module.name().to_string(), module);
    }

    pub fn execute(&self, name: &str, input: &[u8]) -> Result<Vec<u8>, WasmError> {
        let module = self
            .modules
            .get(name)
            .ok_or_else(|| WasmError::ModuleNotFound(name.to_string()))?;

        if input.len() > self.memory_limit || input.len() > module.memory_limit_bytes() {
            return Err(WasmError::MemoryLimitExceeded {
                limit: self.memory_limit.min(module.memory_limit_bytes()),
                requested: input.len(),
            });
        }

        module.execute(input)
    }
}

// Concrete stub module — simulates a CRCI edge computation:
// takes a serialized CrciSignal, returns a priority score (u8)
pub struct PriorityScorer;

impl WasmModule for PriorityScorer {
    fn name(&self) -> &str {
        "priority_scorer"
    }

    fn execute(&self, input: &[u8]) -> Result<Vec<u8>, WasmError> {
        if input.is_empty() {
            return Err(WasmError::InvalidInput("Input cannot be empty".to_string()));
        }
        // Dummy logic: parse severity from first byte (1-5), return score 0-255
        let severity = input[0];
        let score = (severity as u16 * 50).min(255) as u8;
        Ok(vec![score])
    }

    fn memory_limit_bytes(&self) -> usize {
        4 * 1024 * 1024 // 4MB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_priority_scorer_valid() {
        let mut runtime = WasmRuntime::new(10 * 1024 * 1024, 100);
        runtime.register(Box::new(PriorityScorer));

        let result = runtime.execute("priority_scorer", &[4, 0, 0, 0, 0]);
        assert_eq!(result, Ok(vec![200]));
    }

    #[test]
    fn test_execute_oversized_input() {
        let mut runtime = WasmRuntime::new(100, 100); // 100 bytes limit
        runtime.register(Box::new(PriorityScorer));

        let input = vec![0; 200];
        let result = runtime.execute("priority_scorer", &input);
        assert_eq!(
            result,
            Err(WasmError::MemoryLimitExceeded {
                limit: 100,
                requested: 200
            })
        );
    }

    #[test]
    fn test_execute_unknown_module() {
        let runtime = WasmRuntime::new(10 * 1024 * 1024, 100);
        let result = runtime.execute("unknown_module", &[1, 2, 3]);
        assert_eq!(
            result,
            Err(WasmError::ModuleNotFound("unknown_module".to_string()))
        );
    }
}
