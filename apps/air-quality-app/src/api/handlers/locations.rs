use crate::{response::ApiResponse, ApiResult};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub device_type: String,
    pub last_seen: String,
}

pub struct LocationStore {
    locations: Vec<Location>,
}

impl LocationStore {
    pub fn new() -> Self {
        Self {
            locations: Vec::new(),
        }
    }

    pub fn add_location(&mut self, location: Location) {
        self.locations.push(location);
    }

    pub fn get_all_locations(&self) -> Vec<Location> {
        self.locations.clone()
    }
}

pub async fn locations_handler(
    State(location_store): State<Arc<LocationStore>>,
) -> ApiResult<Json<ApiResponse<Vec<Location>>>> {
    let locations = location_store.get_all_locations();
    Ok(Json(ApiResponse::success(locations)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_location(id: &str, name: &str) -> Location {
        Location {
            id: id.to_string(),
            name: name.to_string(),
            latitude: 37.7749,
            longitude: -122.4194,
            device_type: "AirGradient ONE".to_string(),
            last_seen: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_locations_handler_multiple_locations() {
        let mut store = LocationStore::new();
        store.add_location(create_test_location("loc-1", "Office"));
        store.add_location(create_test_location("loc-2", "Home"));
        store.add_location(create_test_location("loc-3", "Warehouse"));

        let result = locations_handler(State(Arc::new(store))).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 3);
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_locations_handler_empty() {
        let store = LocationStore::new();

        let result = locations_handler(State(Arc::new(store))).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 0);
    }

    #[tokio::test]
    async fn test_locations_handler_single_location() {
        let mut store = LocationStore::new();
        store.add_location(create_test_location("loc-1", "Office"));

        let result = locations_handler(State(Arc::new(store))).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].id, "loc-1");
        assert_eq!(response.data[0].name, "Office");
    }

    #[test]
    fn test_location_equality() {
        let loc1 = create_test_location("loc-1", "Office");
        let loc2 = create_test_location("loc-1", "Office");

        assert_eq!(loc1.id, loc2.id);
        assert_eq!(loc1.name, loc2.name);
    }
}
