# Test Grafana image with test-specific configuration
FROM grafana/grafana:latest

# Install test utilities
USER root
RUN apk add --no-cache curl jq

# Copy test-specific Grafana configuration
COPY configs/grafana/test-datasources.yml /etc/grafana/provisioning/datasources/datasources.yml
COPY configs/grafana/test-dashboards.yml /etc/grafana/provisioning/dashboards/dashboards.yml
COPY configs/grafana/test-dashboards/ /var/lib/grafana/dashboards/

# Copy test configuration
COPY configs/grafana/grafana-test.ini /etc/grafana/grafana.ini

# Create necessary directories and set permissions
RUN mkdir -p /var/lib/grafana/plugins /var/log/grafana /test-dashboards && \
    chown -R grafana:grafana /var/lib/grafana /var/log/grafana /test-dashboards /etc/grafana

# Switch back to grafana user
USER grafana

# Environment variables for testing
ENV GF_SECURITY_ADMIN_PASSWORD=test_admin_123
ENV GF_LOG_LEVEL=debug
ENV GF_INSTALL_PLUGINS=""
ENV GF_PATHS_LOGS=/var/log/grafana
ENV GF_PATHS_DATA=/var/lib/grafana
ENV GF_PATHS_PLUGINS=/var/lib/grafana/plugins

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

# Default command
CMD ["/run.sh"]