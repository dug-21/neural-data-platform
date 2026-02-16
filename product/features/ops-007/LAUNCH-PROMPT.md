# Implementation Launch Prompt: ops-007

## Proposed Prompt
> Implement ops-007: Integration Testbed Framework
> GitHub Issue: #21
> Brief: product/features/ops-007/IMPLEMENTATION-BRIEF.md
> Pattern IDs from planning: 3 (deploy-sh-ndp-dispatch), 16 (ops-003-phase3-internal-consolidation), 14 (config-file-retirement), 4 (crate-validate-migration), 2 (ndp-cli-subcommand), 17 (testbed-runner-composition), 18 (etcd-sync-fix), 19 (gold-ddl-config-path), 20 (mqtt-injection), 21 (clean-slate-reset), 22 (manifest-per-testbed), 23 (assertion-library)
> Constraints: Shell scripts only -- NO new Rust code. All fixes in deploy.sh + new scripts in tests/integration/.
> Wave structure: Wave 1 (env completion + MQTT injection), Wave 2 (testbed framework + manifests), Wave 3 (validation helpers + end-to-end smoke)

## Reminders for User
- Review ALIGNMENT-REPORT.md for any variances (none found -- all PASS)
- Verify acceptance criteria in SCOPE.md (12 ACs, all mapped)
- deploy.sh etcd sync fix (WS1-02) and Gold DDL path fix (WS1-03) are production fixes that cascade to prod
- The tests/integration/ directory does not exist yet -- Wave 1 creates it
- Integration compose file already exists at docker-compose.integration.yml

## Gotchas Discovered During Planning
- **Container naming**: Integration containers use `-integration` suffix (e.g., `ndp-timescaledb-integration`). Verify actual container names from docker-compose.integration.yml before hardcoding.
- **mosquitto_pub inside container**: The mosquitto container image includes mosquitto_pub. Use `docker exec` to invoke it, not host-level install.
- **CONFIG_STREAMS_DIR derivation**: The Gold DDL fix (4 lines, not 1) uses `dirname "$CONFIG_STREAMS_DIR"` which gives `config/base` or `config/integration/base`. This is passed to `ndp gold` (the CLI, not the legacy `ndp-gold-ddl` binary). The CLI's gold.rs takes the parent of `--config-dir` to find domain configs. Verify: `handle_gold_table()` line ~1976 + `handle_domain()` lines ~2094, ~2120, ~2139.
- **domain.json intelligence block**: The integration domain config may be missing the `intelligence` block added in fe-004. Check the schema at `config/schemas/domain.schema.json` for required fields.
- **Silver ETL is continuous**: air-quality-app handles ETL continuously (silver-etl batch app is deprecated). After MQTT injection, data should appear in Silver within seconds, not requiring manual trigger.
- **Stress testbed duration**: 30 minutes at 10 msg/sec = 18,000 messages. Ensure the stress manifest allocates enough resources and the template has enough variety to avoid duplicate detection.
- **etcd sync uses docker exec -i**: Per Pattern ID 46 (docker-exec-stdin-unreliable), prefer passing data as arguments rather than piping through stdin. The etcdctl put in the sync fix should use argument form.
