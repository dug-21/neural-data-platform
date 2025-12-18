# Grafana Provisioning Configuration

**Feature**: DP-001 (Data Platform - Silver Layer)
**Component**: Grafana Visualization Layer
**Created**: 2025-12-18

## Overview

Grafana provisioning enables automated configuration of datasources and dashboards on container startup. This document describes the provisioning approach for the Neural Data Platform's visualization layer.

## Directory Structure

```
config/grafana/
├── grafana.ini                           # Main Grafana configuration
└── provisioning/
    ├── datasources/
    │   └── duckdb.yaml                   # DuckDB datasource configuration
    └── dashboards/
        └── default.yaml                  # Dashboard provider configuration
```

Dashboard JSON files are stored separately:
```
config/dashboards/
└── indoor-air-quality.json               # Dashboard definitions
```

## Datasource Strategy

### DuckDB via SQLite Plugin

**Plugin**: `frser-sqlite-datasource`
**Rationale**:
- Native SQLite plugin works with DuckDB files on ARM64
- MotherDuck plugin may have ARM64 compatibility issues
- Direct file access is simpler for embedded deployments

**Configuration** (`datasources/duckdb.yaml`):
```yaml
apiVersion: 1
datasources:
  - name: DuckDB
    type: frser-sqlite-datasource
    access: proxy
    url: /data/duckdb/analytics.duckdb
    isDefault: true
    editable: false
    jsonData:
      path: /data/duckdb/analytics.duckdb
```

**Volume Mapping**:
```yaml
volumes:
  - ./data/duckdb:/data/duckdb:ro  # Read-only for safety
```

### Query Compatibility

The SQLite plugin expects SQLite-compatible SQL. DuckDB's SQL is largely compatible, but note:

**Supported**:
- Standard SELECT queries
- JOINs, GROUP BY, HAVING
- Window functions
- CTEs (WITH clauses)
- Date/time functions

**Potential Issues**:
- DuckDB-specific functions (e.g., `list_aggregate`)
- Column types (may display as generic types)
- Extensions (not available)

**Workaround**: Pre-materialize complex queries as views in DuckDB:
```sql
CREATE VIEW grafana_indoor_readings AS
SELECT
    timestamp,
    temperature,
    humidity,
    pm25,
    co2
FROM indoor_readings
ORDER BY timestamp DESC;
```

## Dashboard Provisioning

### Provider Configuration

**File**: `provisioning/dashboards/default.yaml`

```yaml
apiVersion: 1
providers:
  - name: 'default'
    orgId: 1
    folder: 'Neural Data Platform'      # Grafana folder for organization
    folderUid: 'ndp'
    type: file
    disableDeletion: false              # Allow deletion via UI
    updateIntervalSeconds: 30           # Check for changes every 30s
    allowUiUpdates: true                # Allow editing in Grafana UI
    options:
      path: /var/lib/grafana/dashboards
```

### Dashboard Update Workflow

**Automatic Discovery**:
1. Dashboard JSON files placed in `/var/lib/grafana/dashboards/`
2. Grafana scans directory every 30 seconds
3. New/modified dashboards automatically imported
4. Changes appear without restart

**Volume Mapping**:
```yaml
volumes:
  - ./config/dashboards:/var/lib/grafana/dashboards:ro
```

**UI Updates**:
- `allowUiUpdates: true` enables editing dashboards in Grafana
- Changes made in UI are temporary (not persisted to files)
- To persist changes: Export JSON from UI → Save to `config/dashboards/`

### Dashboard Export/Import Cycle

**Export from UI**:
1. Edit dashboard in Grafana
2. Share → Export → Save JSON locally
3. Copy to `config/dashboards/`
4. Git commit for version control

**Import to Provisioning**:
1. Place JSON in `config/dashboards/`
2. Wait 30 seconds for auto-discovery
3. Dashboard appears in "Neural Data Platform" folder

## Configuration Files

### grafana.ini

**Key Settings**:

```ini
[server]
http_port = 3000                        # Standard Grafana port

[security]
admin_user = admin                      # Default admin credentials
admin_password = admin                  # (Change in production!)
disable_initial_admin_creation = false

[auth.anonymous]
enabled = true                          # Allow unauthenticated viewing
org_name = Main Org.
org_role = Viewer                       # Read-only for anonymous users

[dashboards]
default_home_dashboard_path = /var/lib/grafana/dashboards/indoor-air-quality.json

[plugins]
allow_loading_unsigned_plugins = frser-sqlite-datasource  # Required for SQLite plugin

[log]
mode = console                          # Docker-friendly logging
level = info
```

**Security Note**: Default credentials are acceptable for local development. Production deployments should:
- Use strong passwords via environment variables
- Disable anonymous access
- Enable HTTPS
- Configure proper authentication (OAuth, LDAP, etc.)

## Docker Integration

### docker-compose.yml Addition

```yaml
services:
  grafana:
    image: grafana/grafana:latest
    container_name: ndp-grafana
    ports:
      - "3000:3000"
    environment:
      - GF_PATHS_CONFIG=/etc/grafana/grafana.ini
      - GF_INSTALL_PLUGINS=frser-sqlite-datasource
    volumes:
      # Configuration
      - ./config/grafana/grafana.ini:/etc/grafana/grafana.ini:ro
      - ./config/grafana/provisioning:/etc/grafana/provisioning:ro

      # Dashboards
      - ./config/dashboards:/var/lib/grafana/dashboards:ro

      # Data access
      - ./data/duckdb:/data/duckdb:ro

      # Persistent storage
      - grafana-data:/var/lib/grafana
    depends_on:
      - duckdb-service  # If applicable
    restart: unless-stopped
    networks:
      - ndp-network

volumes:
  grafana-data:
    driver: local
```

### Plugin Installation

The `GF_INSTALL_PLUGINS` environment variable automatically installs the SQLite datasource plugin on container startup:

```yaml
environment:
  - GF_INSTALL_PLUGINS=frser-sqlite-datasource
```

**First Startup**: May take 30-60 seconds to download and install plugin.

## Alternative: MotherDuck Plugin (Future)

If ARM64-compatible MotherDuck plugin becomes available:

**Plugin**: `motherduck-duckdb-datasource`

**Configuration**:
```yaml
apiVersion: 1
datasources:
  - name: DuckDB
    type: motherduck-duckdb-datasource
    access: proxy
    url: /data/duckdb/analytics.duckdb
    isDefault: true
    jsonData:
      mode: file
      path: /data/duckdb/analytics.duckdb
```

**Advantages**:
- Native DuckDB support
- Full DuckDB function compatibility
- Better type handling

**Migration Path**: Change datasource type in `duckdb.yaml`, dashboard queries remain unchanged.

## Testing Provisioning

### Verify Datasource

```bash
# Check Grafana logs for provisioning
docker logs ndp-grafana | grep -i provision

# Expected output:
# t=... lvl=info msg="Provisioning datasources" ...
# t=... lvl=info msg="Provisioning dashboards" ...
```

### Test Datasource Connection

1. Open Grafana: http://localhost:3000
2. Configuration → Data Sources
3. Click "DuckDB"
4. Click "Test" button
5. Should see "Data source is working"

### Test Query

```sql
SELECT timestamp, temperature
FROM indoor_readings
ORDER BY timestamp DESC
LIMIT 10;
```

Expected: Recent readings with timestamp and temperature columns.

## Troubleshooting

### Plugin Not Loading

**Symptom**: "Unknown datasource type: frser-sqlite-datasource"

**Solution**:
```bash
# Check plugin installation
docker exec ndp-grafana ls /var/lib/grafana/plugins/

# Manually install if needed
docker exec ndp-grafana grafana-cli plugins install frser-sqlite-datasource
docker restart ndp-grafana
```

### Dashboard Not Appearing

**Check**:
1. Dashboard JSON syntax is valid
2. File is in `/var/lib/grafana/dashboards/` (inside container)
3. File permissions allow Grafana to read
4. Check Grafana logs for errors

**Debug**:
```bash
# Verify volume mount
docker exec ndp-grafana ls -l /var/lib/grafana/dashboards/

# Check provisioning logs
docker logs ndp-grafana | grep dashboard
```

### Database File Not Found

**Symptom**: "unable to open database file"

**Solution**:
```bash
# Verify DuckDB file exists
ls -l data/duckdb/analytics.duckdb

# Verify volume mount
docker exec ndp-grafana ls -l /data/duckdb/

# Check file permissions (must be readable)
chmod 644 data/duckdb/analytics.duckdb
```

## Best Practices

### Configuration Management

1. **Version Control**: Commit all provisioning files to Git
2. **Secrets Management**: Use environment variables for passwords
3. **Documentation**: Keep provisioning docs updated with changes

### Dashboard Development

1. **Develop in UI**: Edit dashboards in Grafana for rapid iteration
2. **Export Regularly**: Save working versions to JSON
3. **Version Control**: Commit stable dashboard versions
4. **Test Queries**: Validate SQL against DuckDB compatibility

### Deployment

1. **Staging First**: Test provisioning changes in staging environment
2. **Rollback Plan**: Keep previous dashboard versions in Git
3. **Monitor Logs**: Check Grafana logs after provisioning changes

## Related Documentation

- **Dashboard Design**: `DASHBOARD_DESIGN.md` (panel layouts, queries)
- **DuckDB Schema**: `../architecture/DUCKDB_SCHEMA.md`
- **Docker Deployment**: `../completion/DEPLOYMENT.md`

## Future Enhancements

1. **Multi-Datasource**: Add Prometheus for application metrics
2. **Alerting**: Configure Grafana alerting rules via provisioning
3. **Organizations**: Separate dashboards by user roles
4. **Plugins**: Add more visualization plugins (Plotly, D3.js)
5. **MotherDuck**: Migrate to native DuckDB plugin when ARM64-ready

---

**Status**: Provisioning configuration ready for Docker integration
**Next Steps**: Create dashboard JSON files, test with Docker Compose
