# Test Prometheus image with test-specific configuration
FROM prom/prometheus:latest

# Copy test-specific Prometheus configuration
COPY configs/prometheus/prometheus-test.yml /etc/prometheus/prometheus.yml
COPY configs/prometheus/alerts-test.yml /etc/prometheus/alerts.yml

# Create test user (Prometheus runs as nobody by default)
USER root
RUN mkdir -p /prometheus-test && \
    chown nobody:nogroup /prometheus-test
USER nobody

# Test environment configuration with shorter retention and scrape intervals
VOLUME ["/prometheus-test"]

# Expose port
EXPOSE 9090

# Health check
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:9090/-/healthy || exit 1

# Default command with test settings
CMD ["--config.file=/etc/prometheus/prometheus.yml", \
     "--storage.tsdb.path=/prometheus-test", \
     "--storage.tsdb.retention.time=7d", \
     "--web.enable-lifecycle", \
     "--log.level=debug"]