# ops-007 Acceptance Criteria Map

| AC-ID | Description | Verification Method | Verification Detail | Status |
|-------|-------------|--------------------|--------------------|--------|
| AC-01 | Integration config has layer parity -- at least 1 representative for MQTT, Silver, Gold, Domain, Intelligence | file-check | `ls config/integration/base/streams/` has MQTT stream; `cat config/integration/domains/indoor-air-quality/domain.json` has intelligence block; stream configs have gold ETL sections | PENDING |
| AC-02 | `DEPLOY_ENV=integration ./deploy.sh apply .../smoke/manifest.json` deploys successfully | shell | `DEPLOY_ENV=integration ./deploy/pi/deploy.sh apply tests/integration/testbeds/smoke/manifest.json; echo $?` returns 0 | PENDING |
| AC-03 | Domain config reaches etcd via both `sync-domains` AND manifest apply | shell | `DEPLOY_ENV=integration ./deploy/pi/deploy.sh sync-domains && docker exec ndp-etcd-integration etcdctl get /domains/indoor-air-quality/config --print-value-only` returns valid JSON | PENDING |
| AC-04 | Gold DDL generation uses correct config path in integration mode | grep | `grep -n 'config-dir.*config/base' deploy/pi/deploy.sh` returns no matches (all 4 refs in handle_gold_table + handle_domain replaced with `$(dirname "$CONFIG_STREAMS_DIR")`) | PENDING |
| AC-05 | Smoke testbed: inject 10 MQTT messages, data appears in Silver within 2 minutes | shell | `./tests/integration/run-testbed.sh smoke` -- assert_silver_rows returns PASS with count >= 1 | PENDING |
| AC-06 | Smoke testbed: validate.sh returns exit 0 on a healthy, data-populated stack | shell | `./tests/integration/run-testbed.sh smoke; echo $?` returns 0 | PENDING |
| AC-07 | Stress testbed: 10 msg/sec for 30 minutes, RSS stays within configured bounds | shell | `./tests/integration/run-testbed.sh stress --timeout 1800` -- assert_container_rss_below returns PASS | PENDING |
| AC-08 | Message templates produce valid JSON matching stream config field definitions | shell | `head -1 tests/integration/fixtures/mqtt/airgradient.jsonl | jq .` succeeds; fields match config/integration/base/streams/air-quality/config.json field list | PENDING |
| AC-09 | `tests/integration/run-testbed.sh smoke` runs end-to-end from clean slate | shell | `./tests/integration/run-testbed.sh smoke; echo $?` returns 0 after docker compose down -v | PENDING |
| AC-10 | Environment prep includes clean database step (test-only, not in production code) | grep | `grep -n 'down -v' tests/integration/lib/prep.sh` finds clean_slate function; `grep -rn 'down -v' deploy/pi/deploy.sh` returns nothing | PENDING |
| AC-11 | Intelligence daemon starts, reads domain config from etcd, and is reachable in integration | shell | `docker compose -f docker-compose.integration.yml --profile intelligence up -d ndp-intelligence && docker exec ndp-intelligence-integration curl -s localhost:8080/health` returns healthy | PENDING |
| AC-12 | Feature testbed convention documented -- `product/features/{id}/testbed/` structure | file-check | IMPLEMENTATION-BRIEF.md contains "Feature Testbed Convention" section with directory structure | PENDING |
