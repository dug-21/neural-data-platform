// Swarm orchestration WASM interfaces
use wasm_bindgen::prelude::*;
use ruv_swarm_core::{Swarm, Topology};

#[wasm_bindgen]
pub struct WasmSwarm {
    inner: Swarm,
}

#[wasm_bindgen]
impl WasmSwarm {
    #[wasm_bindgen(constructor)]
    pub fn new(max_agents: usize) -> WasmSwarm {
        WasmSwarm {
            inner: Swarm::new(max_agents),
        }
    }
    
    #[wasm_bindgen]
    pub fn set_topology(&mut self, topology: String) -> Result<(), JsValue> {
        let topo = match topology.as_str() {
            "mesh" => Topology::Mesh,
            "star" => Topology::Star,
            "ring" => Topology::Ring,
            "hierarchical" => Topology::Hierarchical,
            _ => return Err(JsValue::from_str(&format!("Unknown topology: {}", topology))),
        };
        
        self.inner.topology = topo;
        Ok(())
    }
    
    #[wasm_bindgen]
    pub fn get_agent_count(&self) -> usize {
        self.inner.agents.len()
    }
    
    #[wasm_bindgen]
    pub fn get_info(&self) -> JsValue {
        let info = serde_json::json!({
            "agent_count": self.get_agent_count(),
            "max_agents": self.inner.max_agents,
            "topology": format!("{:?}", self.inner.topology),
        });
        
        serde_wasm_bindgen::to_value(&info).unwrap()
    }
}