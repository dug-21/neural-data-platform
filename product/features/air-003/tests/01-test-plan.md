# AIR-003 Test Plan (etcd Approach)

## Test Categories

### 1. Unit Tests (config-client crate)

| Test | Description | Status |
|------|-------------|--------|
| `test_connect_to_etcd` | Basic connection | Pending |
| `test_get_set_value` | Get/set operations | Pending |
| `test_delete_value` | Delete operation | Pending |
| `test_list_keys` | List keys with prefix | Pending |
| `test_env_override` | Env var takes precedence | Pending |
| `test_not_found_error` | Proper error on missing key | Pending |

### 2. Integration Tests

| Test | Description | Status |
|------|-------------|--------|
| `test_load_config_from_etcd` | Full config load | Pending |
| `test_watch_config_changes` | Real-time updates | Pending |
| `test_gitops_sync` | YAML to etcd sync | Pending |
| `test_env_overlay` | Environment-specific config | Pending |
| `test_graceful_fallback` | Fallback when etcd unavailable | Pending |

### 3. End-to-End Tests

| Test | Description | Status |
|------|-------------|--------|
| `test_app_starts_with_etcd_config` | App uses etcd config | Pending |
| `test_hot_reload` | Config change affects running app | Pending |
| `test_full_flow` | Git → etcd → app | Pending |

## Test Commands

```bash
# Unit tests (no etcd required)
cargo test -p config-client --lib

# Integration tests (requires etcd)
docker compose up -d etcd
cargo test -p config-client --test '*' -- --ignored
cargo test -p air-quality-app --test etcd_config_test -- --ignored

# End-to-end test
./scripts/sync-config-to-etcd.sh development
ETCD_ENDPOINT=http://localhost:2379 cargo run -p air-quality-app &
curl http://localhost:8080/health
```

## Success Criteria

- [ ] etcd container starts and is healthy
- [ ] config-client can connect and perform CRUD
- [ ] GitOps sync populates etcd from YAML files
- [ ] air-quality-app loads config from etcd
- [ ] Watch mechanism detects config changes
- [ ] Environment variables override etcd values
- [ ] Graceful fallback when etcd unavailable
