// Task management WASM interfaces
use wasm_bindgen::prelude::*;
use ruv_swarm_core::{Task, TaskPriority};

#[wasm_bindgen]
pub struct WasmTask {
    inner: Task,
}

#[wasm_bindgen]
impl WasmTask {
    #[wasm_bindgen(constructor)]
    pub fn new(name: String, description: String) -> WasmTask {
        WasmTask {
            inner: Task::new(name, description),
        }
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
    pub fn set_priority(&mut self, priority: String) -> Result<(), JsValue> {
        let prio = match priority.as_str() {
            "low" => TaskPriority::Low,
            "medium" => TaskPriority::Medium,
            "high" => TaskPriority::High,
            "critical" => TaskPriority::Critical,
            _ => return Err(JsValue::from_str(&format!("Unknown priority: {}", priority))),
        };
        
        self.inner.priority = prio;
        Ok(())
    }
    
    #[wasm_bindgen]
    pub fn get_info(&self) -> JsValue {
        let info = serde_json::json!({
            "id": self.inner.id,
            "name": self.inner.name,
            "description": self.inner.description,
            "priority": format!("{:?}", self.inner.priority),
            "status": format!("{:?}", self.inner.status),
        });
        
        serde_wasm_bindgen::to_value(&info).unwrap()
    }
}