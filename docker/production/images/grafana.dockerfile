# Grafana with baked-in dashboards and datasources
FROM grafana/grafana:latest

# Copy provisioning configurations
COPY --chown=472:472 configs/grafana/datasources/datasources.yml /etc/grafana/provisioning/datasources/
COPY --chown=472:472 configs/grafana/provisioning/dashboards/dashboard.yml /etc/grafana/provisioning/dashboards/

# Copy dashboards to temp location (will be copied to volume at runtime)
COPY --chown=472:472 configs/grafana/dashboards/*.json /tmp/dashboards/

# Copy custom entrypoint with correct permissions
COPY --chmod=755 scripts/grafana-entrypoint.sh /usr/local/bin/grafana-entrypoint.sh

# Environment variables for configuration
ENV GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD}
ENV GF_USERS_ALLOW_SIGN_UP=false
ENV GF_ANALYTICS_REPORTING_ENABLED=false
ENV GF_ANALYTICS_CHECK_FOR_UPDATES=false

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:3000/api/health || exit 1

# Use custom entrypoint
ENTRYPOINT ["/usr/local/bin/grafana-entrypoint.sh"]