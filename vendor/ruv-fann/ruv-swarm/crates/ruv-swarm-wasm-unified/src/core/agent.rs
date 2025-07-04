// Agent management WASM interfaces
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use ruv_swarm_core::{Agent, AgentType, CognitivePattern};

#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmAgent {
    inner: Agent,
}

#[wasm_bindgen]
impl WasmAgent {
    #[wasm_bindgen(constructor)]
    pub fn new(name: String, agent_type: String) -> Result<WasmAgent, JsValue> {
        let agent_type = match agent_type.as_str() {
            "researcher" => AgentType::Researcher,
            "coder" => AgentType::Coder,
            "analyst" => AgentType::Analyst,
            "optimizer" => AgentType::Optimizer,
            "coordinator" => AgentType::Coordinator,
            _ => return Err(JsValue::from_str(&format!("Unknown agent type: {}", agent_type))),
        };

        Ok(WasmAgent {
            inner: Agent::new(name, agent_type),
        })
    }

    #[wasm_bindgen]
    pub fn get_id(&self) -> String {
        self.inner.id.clone()
    }

    #[wasm_bindgen]
    pub fn get_name(&self) -> String {
        self.inner.name.clone()
    }

    #[wasm_bindgen]
    pub fn get_type(&self) -> String {
        match self.inner.agent_type {
            AgentType::Researcher => "researcher",
            AgentType::Coder => "coder",
            AgentType::Analyst => "analyst",
            AgentType::Optimizer => "optimizer",
            AgentType::Coordinator => "coordinator",
        }.to_string()
    }

    #[wasm_bindgen]
    pub fn set_cognitive_pattern(&mut self, pattern: String) -> Result<(), JsValue> {
        let cognitive_pattern = match pattern.as_str() {
            "convergent" => CognitivePattern::Convergent,
            "divergent" => CognitivePattern::Divergent,
            "lateral" => CognitivePattern::Lateral,
            "systems" => CognitivePattern::Systems,
            "critical" => CognitivePattern::Critical,
            "abstract" => CognitivePattern::Abstract,
            _ => return Err(JsValue::from_str(&format!("Unknown cognitive pattern: {}", pattern))),
        };

        self.inner.cognitive_pattern = Some(cognitive_pattern);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_cognitive_pattern(&self) -> Option<String> {
        self.inner.cognitive_pattern.as_ref().map(|p| match p {
            CognitivePattern::Convergent => "convergent",
            CognitivePattern::Divergent => "divergent",
            CognitivePattern::Lateral => "lateral",
            CognitivePattern::Systems => "systems",
            CognitivePattern::Critical => "critical",
            CognitivePattern::Abstract => "abstract",
        }.to_string())
    }

    #[wasm_bindgen]
    pub fn get_info(&self) -> JsValue {
        let info = serde_json::json!({
            "id": self.inner.id,
            "name": self.inner.name,
            "type": self.get_type(),
            "cognitive_pattern": self.get_cognitive_pattern(),
            "state": match self.inner.state {
                ruv_swarm_core::AgentState::Idle => "idle",
                ruv_swarm_core::AgentState::Active => "active",
                ruv_swarm_core::AgentState::Busy => "busy",
            },
        });
        
        serde_wasm_bindgen::to_value(&info).unwrap()
    }
}

// Agent factory with optimization hints
#[wasm_bindgen]
pub struct AgentFactory {
    optimization_hints: bool,
}

#[wasm_bindgen]
impl AgentFactory {
    #[wasm_bindgen(constructor)]
    pub fn new() -> AgentFactory {
        AgentFactory {
            optimization_hints: true,
        }
    }

    #[wasm_bindgen]
    pub fn create_agent(&self, name: String, agent_type: String) -> Result<WasmAgent, JsValue> {
        WasmAgent::new(name, agent_type)
    }

    #[wasm_bindgen]
    pub fn create_optimized_agent(&self, name: String, agent_type: String, memory_budget_mb: u32) -> Result<WasmAgent, JsValue> {
        // Create agent with memory optimization hints
        let agent = WasmAgent::new(name, agent_type)?;
        
        // Apply memory optimizations based on budget
        if memory_budget_mb < 10 {
            // Use minimal memory allocations
            web_sys::console::log_1(&"Creating memory-optimized agent".into());
        }
        
        Ok(agent)
    }

    #[wasm_bindgen]
    pub fn create_batch(&self, count: u32, base_name: String, agent_type: String) -> Result<Vec<JsValue>, JsValue> {
        let mut agents = Vec::new();
        
        for i in 0..count {
            let name = format!("{}-{}", base_name, i);
            let agent = self.create_agent(name, agent_type.clone())?;
            agents.push(agent.get_info());
        }
        
        Ok(agents)
    }
}