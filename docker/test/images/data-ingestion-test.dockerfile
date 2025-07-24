# Test data ingestion image with mock providers
FROM python:3.11-slim

# Install system dependencies + test tools
RUN apt-get update && apt-get install -y \
    gcc \
    g++ \
    postgresql-client \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash ingester

# Create app directory
WORKDIR /app

# Copy requirements first for better caching
COPY data_ingestion/requirements.txt ./
COPY docker/test/configs/data-ingestion/requirements-test.txt ./requirements-test.txt

# Install dependencies including test packages
RUN pip install --no-cache-dir -r requirements.txt -r requirements-test.txt

# Copy application code
COPY data_ingestion/ ./

# Copy test-specific code
COPY docker/test/configs/data-ingestion/mock_providers.py ./providers/mock_providers.py
COPY docker/test/configs/data-ingestion/test_config.py ./test_config.py

# Copy test scripts
COPY docker/test/scripts/test-data-ingestion.py /usr/local/bin/test-data-ingestion.py
COPY docker/test/scripts/start-test-data-ingestion.sh /usr/local/bin/start-test-data-ingestion.sh
RUN chmod +x /usr/local/bin/test-data-ingestion.py /usr/local/bin/start-test-data-ingestion.sh

# Create necessary directories
RUN mkdir -p /var/log/data-ingestion /var/lib/data-ingestion /test-fixtures && \
    chown -R ingester:ingester /app /var/log/data-ingestion /var/lib/data-ingestion /test-fixtures

# Switch to non-root user
USER ingester

# Environment variables for testing
ENV PYTHONUNBUFFERED=1
ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONPATH=/app
ENV LOG_LEVEL=DEBUG
ENV UPDATE_INTERVAL=5
ENV BATCH_SIZE=10
ENV TESTING_MODE=true
ENV MOCK_MODE=true
ENV TEST_DATA_ENABLED=true

# Expose ports (same as production but will be mapped differently)
EXPOSE 8001 9090

# Health check - faster for testing
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=3 \
    CMD python -c "import requests; requests.get('http://localhost:8001/health', timeout=5).raise_for_status()" || exit 1

# Use test wrapper script
ENTRYPOINT ["/usr/local/bin/start-test-data-ingestion.sh"]
CMD []