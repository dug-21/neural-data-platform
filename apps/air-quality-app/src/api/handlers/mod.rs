pub mod alerts;
pub mod forecast;
pub mod health;
pub mod locations;
pub mod readings;

pub use alerts::alerts_handler;
pub use forecast::forecast_handler;
pub use health::health_handler;
pub use locations::locations_handler;
pub use readings::{aggregate_handler, latest_readings_handler, readings_handler};
