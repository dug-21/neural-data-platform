//! FFI Wrapper for JS/WASM DAA Service Communication
//!
//! This module provides C-compatible FFI functions for cross-boundary
//! communication between Rust and the JS/WASM DAA service.

use serde_json::{json, Value};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::slice;

use super::daa_service::{DAAMessage, DAATradingDecision};

/// Result wrapper for FFI calls
#[repr(C)]
pub struct FFIResult {
    pub success: bool,
    pub data: *mut c_char,
    pub error: *mut c_char,
}

impl FFIResult {
    fn success(data: String) -> Self {
        Self {
            success: true,
            data: CString::new(data).unwrap().into_raw(),
            error: std::ptr::null_mut(),
        }
    }

    fn error(error: String) -> Self {
        Self {
            success: false,
            data: std::ptr::null_mut(),
            error: CString::new(error).unwrap().into_raw(),
        }
    }
}

/// Free FFI result memory
#[no_mangle]
pub extern "C" fn ffi_free_result(result: FFIResult) {
    unsafe {
        if !result.data.is_null() {
            let _ = CString::from_raw(result.data);
        }
        if !result.error.is_null() {
            let _ = CString::from_raw(result.error);
        }
    }
}

/// Create a DAA analysis request from market data
#[no_mangle]
pub extern "C" fn ffi_create_analysis_request(
    symbol: *const c_char,
    data_json: *const c_char,
    analysis_type: *const c_char,
) -> FFIResult {
    let result: Result<String, &str> = (|| {
        // Parse inputs
        let symbol_str = unsafe { CStr::from_ptr(symbol) }
            .to_str()
            .map_err(|_| "Invalid symbol string")?;

        let data_str = unsafe { CStr::from_ptr(data_json) }
            .to_str()
            .map_err(|_| "Invalid data JSON")?;

        let analysis_type_str = unsafe { CStr::from_ptr(analysis_type) }
            .to_str()
            .map_err(|_| "Invalid analysis type")?;

        // Parse market data
        let data_value: Value =
            serde_json::from_str(data_str).map_err(|_| "Failed to parse market data JSON")?;

        // Create request message
        let request = json!({
            "agent_id": "neural-trader-ffi",
            "message_type": "analysis_request",
            "payload": {
                "symbol": symbol_str,
                "analysis_type": analysis_type_str,
                "data": data_value,
                "parameters": {
                    "include_indicators": true,
                    "include_risk_assessment": true,
                }
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "correlation_id": uuid::Uuid::new_v4().to_string(),
        });

        Ok(request.to_string())
    })();

    match result {
        Ok(data) => FFIResult::success(data),
        Err(e) => FFIResult::error(e.to_string()),
    }
}

/// Parse a trading decision from DAA message
#[no_mangle]
pub extern "C" fn ffi_parse_trading_decision(message_json: *const c_char) -> FFIResult {
    let result: Result<String, &str> = (|| {
        let message_str = unsafe { CStr::from_ptr(message_json) }
            .to_str()
            .map_err(|_| "Invalid message JSON")?;

        let message: DAAMessage =
            serde_json::from_str(message_str).map_err(|_| "Failed to parse DAA message")?;

        let decision: DAATradingDecision = serde_json::from_value(message.payload)
            .map_err(|_| "Failed to parse trading decision")?;

        Ok(serde_json::to_string(&decision).unwrap())
    })();

    match result {
        Ok(data) => FFIResult::success(data),
        Err(e) => FFIResult::error(e.to_string()),
    }
}

/// Convert market data array to DAA format
#[no_mangle]
pub extern "C" fn ffi_convert_to_daa_format(
    data_ptr: *const f64,
    data_len: usize,
    fields_per_record: usize,
) -> FFIResult {
    let result: Result<String, &str> = (|| {
        if data_ptr.is_null() || data_len == 0 || fields_per_record == 0 {
            return Err("Invalid data parameters");
        }

        let data_slice = unsafe { slice::from_raw_parts(data_ptr, data_len) };
        let num_records = data_len / fields_per_record;

        let mut records = Vec::new();
        for i in 0..num_records {
            let offset = i * fields_per_record;
            if offset + 7 <= data_len {
                records.push(json!({
                    "timestamp": data_slice[offset] as i64,
                    "open": data_slice[offset + 1],
                    "high": data_slice[offset + 2],
                    "low": data_slice[offset + 3],
                    "close": data_slice[offset + 4],
                    "volume": data_slice[offset + 5],
                    "indicator_value": data_slice[offset + 6],
                }));
            }
        }

        let result = json!({
            "type": "market_data",
            "data": records,
            "source": "ffi",
            "version": "1.0"
        });

        Ok(result.to_string())
    })();

    match result {
        Ok(data) => FFIResult::success(data),
        Err(e) => FFIResult::error(e.to_string()),
    }
}

/// Create performance feedback message
#[no_mangle]
pub extern "C" fn ffi_create_performance_feedback(
    decision_id: *const c_char,
    actual_pnl: f64,
    execution_price: f64,
    current_price: f64,
) -> FFIResult {
    let result: Result<String, &str> = (|| {
        let decision_id_str = unsafe { CStr::from_ptr(decision_id) }
            .to_str()
            .map_err(|_| "Invalid decision ID")?;

        let feedback = json!({
            "agent_id": "neural-trader-ffi",
            "message_type": "performance_feedback",
            "payload": {
                "decision_id": decision_id_str,
                "actual_pnl": actual_pnl,
                "execution_price": execution_price,
                "market_snapshot": {
                    "price": current_price,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                },
                "feedback_type": "trade_outcome"
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "correlation_id": decision_id_str,
        });

        Ok(feedback.to_string())
    })();

    match result {
        Ok(data) => FFIResult::success(data),
        Err(e) => FFIResult::error(e.to_string()),
    }
}

/// Opaque handle for cross-boundary state management
#[repr(C)]
pub struct FFIBridgeHandle {
    _private: [u8; 0],
}

/// Create a new bridge handle
#[no_mangle]
pub extern "C" fn ffi_bridge_create() -> *mut FFIBridgeHandle {
    Box::into_raw(Box::new(FFIBridgeHandle { _private: [] }))
}

/// Destroy a bridge handle
#[no_mangle]
pub extern "C" fn ffi_bridge_destroy(handle: *mut FFIBridgeHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_result_creation() {
        let success = FFIResult::success("test data".to_string());
        assert!(success.success);
        assert!(!success.data.is_null());
        assert!(success.error.is_null());
        ffi_free_result(success);

        let error = FFIResult::error("test error".to_string());
        assert!(!error.success);
        assert!(error.data.is_null());
        assert!(!error.error.is_null());
        ffi_free_result(error);
    }
}
