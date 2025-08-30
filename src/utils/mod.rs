//! Utility modules for the neural trader system

pub mod disk_manager;
pub mod market_hours;
pub mod resource_monitor;
pub mod symbol_loader;

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
pub use symbol_loader::{
    load_trading_symbols, load_stock_symbols, load_sector_etf_symbols,
    get_symbol_count, is_sector_etf, get_sector_for_etf,
};