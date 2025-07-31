//! Utility modules for the neural trader system

pub mod disk_manager;
pub mod market_hours;
pub mod resource_monitor;

// Re-export commonly used types
pub use market_hours::{
    Exchange, MarketHours, MarketSession, TrainingWindow,
    MarketIntensity, MarketStatus, VolatilityLevel,
    ResourceAllocationPolicy, EmergencyOverride, EmergencyPriority,
    SchedulingRecommendations, HolidayType,
};
pub use disk_manager::DiskManager;
pub use resource_monitor::{
    ResourceGovernor, ResourceMonitor, ResourceSnapshot, ResourceLimits,
    GovernorConfig, EnforcementMode, ResourceViolation, ViolationSeverity,
};