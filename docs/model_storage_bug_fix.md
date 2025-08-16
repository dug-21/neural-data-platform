# Model Storage Critical Bug Fix

## Issue Description

The model storage system had a critical bug where saving a new model could cause deletion of models of different types. This occurred when the version history enforcement logic was incorrectly deleting from the front of a global queue instead of filtering by model type.

### Symptoms
- Log message: "Deleting old version: healthcare_base_model v1.2.0" when saving real_estate_base_model
- Models of different types being deleted unexpectedly
- Version limits being applied globally instead of per model type

### Root Cause

In `/src/adapters/model_storage.rs` around line 262-266, the original code:

```rust
// Enforce max versions limit
while history.len() > self.config.max_versions_per_model {
    if let Some(old_version) = history.pop_front() {
        self.delete_version(&old_version).await?;
    }
}
```

**Problem**: This logic treated `max_versions_per_model` as a **global limit** across ALL model types, deleting from the front of a shared `VecDeque` without considering model type.

## Fix Implementation

### 1. Changed Logic to Filter by Model Type

```rust
// Enforce max versions limit PER MODEL TYPE (not globally)
let current_model_type = &model_version.model_type;

// Collect all versions of the current model type with their positions
let mut model_type_versions: Vec<(usize, ModelVersion)> = Vec::new();
for (index, version) in history.iter().enumerate() {
    if version.model_type == *current_model_type {
        model_type_versions.push((index, version.clone()));
    }
}

// If we have too many versions of this model type, remove the oldest ones
if model_type_versions.len() > self.config.max_versions_per_model {
    let excess_count = model_type_versions.len() - self.config.max_versions_per_model;
    
    // Sort by timestamp to ensure we delete the oldest versions first
    model_type_versions.sort_by_key(|(_, version)| version.timestamp);
    
    // ... deletion logic for specific model type only
}
```

### 2. Key Changes

1. **Model Type Filtering**: Only considers versions of the same model type when checking limits
2. **Timestamp-based Ordering**: Ensures oldest versions are deleted first within the same model type  
3. **Index-safe Removal**: Removes from VecDeque in reverse order to maintain valid indices
4. **Enhanced Logging**: Added clear logging to show which model versions are being cleaned up and why

### 3. Improved Logging

```rust
info!(
    "Cleaning up old version: {} v{} (exceeded limit of {} for model type)", 
    old_version.model_type, 
    old_version.version,
    self.config.max_versions_per_model
);
```

## Testing

Created comprehensive tests in `/tests/model_storage_fix_test.rs`:

1. **test_model_deletion_by_type**: Verifies that only versions of the same model type are deleted
2. **test_mixed_model_types_cleanup**: Tests interleaved saving of different model types

## Expected Behavior After Fix

- ✅ Healthcare models only delete old healthcare versions
- ✅ Real estate models only delete old real estate versions  
- ✅ Each model type maintains its own version count independently
- ✅ Clear logging shows which specific model versions are being cleaned up
- ✅ No cross-model-type deletions

## Files Modified

1. `/src/adapters/model_storage.rs` - Core fix implementation
2. `/tests/model_storage_fix_test.rs` - Test suite (new file)
3. `/docs/model_storage_bug_fix.md` - This documentation (new file)

## Impact

This fix prevents potential data loss where critical model versions could be accidentally deleted when working with multiple model types. The issue was particularly dangerous in production environments where different teams might be training different types of models concurrently.