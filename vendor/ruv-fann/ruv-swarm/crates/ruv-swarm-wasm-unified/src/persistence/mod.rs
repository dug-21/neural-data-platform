// Persistence layer WASM interfaces
use wasm_bindgen::prelude::*;
use ruv_swarm_persistence::WasmStorage;

#[wasm_bindgen]
pub struct PersistenceManager {
    storage: WasmStorage,
}

#[wasm_bindgen]
impl PersistenceManager {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<PersistenceManager, JsValue> {
        Ok(PersistenceManager {
            storage: WasmStorage::new(),
        })
    }
    
    #[wasm_bindgen]
    pub fn store(&mut self, key: &str, value: JsValue) -> Result<(), JsValue> {
        let data = crate::utils::bridge::DataBridge::json_stringify(&value)?;
        self.storage.set(key, &data);
        Ok(())
    }
    
    #[wasm_bindgen]
    pub fn retrieve(&self, key: &str) -> Result<JsValue, JsValue> {
        if let Some(data) = self.storage.get(key) {
            crate::utils::bridge::DataBridge::json_parse(&data)
        } else {
            Err(JsValue::from_str(&format!("Key not found: {}", key)))
        }
    }
}