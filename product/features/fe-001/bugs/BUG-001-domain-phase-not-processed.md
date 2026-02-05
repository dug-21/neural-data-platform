# BUG-001: Domain Phase Not Processed in Declarative Deploy

**Severity**: Medium
**Status**: Open
**Discovered**: 2026-02-05
**Release**: v1.1.2
**Component**: deploy/pi/deploy.sh

## Summary

The `type: domain` declaration in release manifests is not processed during declarative deploy. Phase 6 (Domains) is skipped, requiring manual `./deploy.sh sync-domains` execution.

## Reproduction

1. Create manifest with domain declaration:
   ```json
   {
     "type": "domain",
     "domain_id": "indoor-air-quality",
     "action": "sync"
   }
   ```

2. Run declarative deploy:
   ```bash
   ./deploy.sh apply .deploy/releases/v1.1.2.manifest.json
   ```

3. **Expected**: Phase 6 (Domains) runs, syncing domain config to `data_dictionary.domains` and `data_dictionary.objectives`

4. **Actual**: Phase 6 is skipped. Deploy log shows:
   - Phase 5: Gold Tables
   - Phase 7: Streams (Phase 6 missing)

## Evidence

Deploy log from v1.1.2:
```
[DEPLOY] Phase 5: Gold Tables (2)
...
[DEPLOY] Phase 7: Streams (2)
```

Post-deploy validation showed 0 domains and 0 objectives until manual sync:
```sql
SELECT count(*) FROM data_dictionary.domains;  -- 0
SELECT count(*) FROM data_dictionary.objectives;  -- 0
```

## Root Cause

The `handle_domain()` function exists in deploy.sh but is not called in the manifest processing loop. The loop handles `tool`, `migration`, `stream`, `gold-tables`, `dictionary` but likely missing the `domain` case.

## Fix

In `deploy.sh`, add domain handling to the manifest processing loop:

```bash
"domain")
    handle_domain "$change"
    ;;
```

Ensure Phase 6 ordering between Gold Tables (5) and Streams (7).

## Workaround

Run manually after deploy:
```bash
./deploy.sh sync-domains
```

## Acceptance Criteria

- [ ] `type: domain` declarations processed in declarative deploy
- [ ] Phase 6 appears in deploy log between Phase 5 and Phase 7
- [ ] Domains and objectives synced without manual intervention
- [ ] Unit test for domain declaration processing
