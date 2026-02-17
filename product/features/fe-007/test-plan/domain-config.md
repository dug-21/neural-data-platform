# fe-007 Test Plan: Domain Config (Schema + Types)

## Unit Tests

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_granger_config_defaults` | `{}` (empty JSON object) | All defaults applied | candidate_count=10, lag_hours=[1,2,4], etc. |
| `test_granger_config_custom` | Full JSON with custom values | Custom values deserialized | Each field matches input |
| `test_granger_config_partial` | JSON with only candidate_count=5 | candidate_count=5, rest defaults | Mixed custom + defaults |
| `test_intelligence_config_without_granger` | Intelligence JSON without granger key | granger = None | Backward compatible |
| `test_intelligence_config_with_granger` | Intelligence JSON with granger block | granger = Some(...) | Deserialized correctly |

## Schema Validation Tests

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_schema_rejects_unknown_granger_field` | `{"granger": {"unknown_field": 1}}` | Schema validation error | additionalProperties: false enforced |
| `test_schema_rejects_invalid_test_method` | `{"granger": {"test_method": "invalid"}}` | Schema validation error | Enum constraint |
| `test_schema_accepts_valid_granger` | Full valid granger block | Passes validation | No errors |
| `test_schema_accepts_intelligence_without_granger` | Intelligence without granger | Passes validation | granger is optional |

## Integration Tests

| Test | Setup | Action | Assertion |
|------|-------|--------|-----------|
| `test_integration_domain_json_valid` | Updated integration domain.json | Validate against schema | Passes |
| `test_etcd_config_reload_granger` | Store granger config in etcd, change value | Reload config | New values reflected |
