# Prometheus with baked-in configuration
FROM prom/prometheus:latest

# Copy configuration
COPY configs/prometheus/prometheus.yml /etc/prometheus/prometheus.yml
COPY configs/prometheus/alerts.yml /etc/prometheus/alerts.yml
COPY configs/prometheus/neural_prediction_alerts.yml /etc/prometheus/neural_prediction_alerts.yml

# Validate configuration
RUN promtool check config /etc/prometheus/prometheus.yml

# Use default user (nobody)
USER nobody

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:9090/-/healthy || exit 1