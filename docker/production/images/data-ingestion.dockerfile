# Production data ingestion image
FROM python:3.11-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    gcc \
    g++ \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash ingester

# Create app directory
WORKDIR /app

# Copy requirements first for better caching
COPY data_ingestion/requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt

# Copy application code
COPY data_ingestion/ ./

# Copy startup script
COPY docker/production/scripts/start-data-ingestion.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/start-data-ingestion.sh

# Create necessary directories
RUN mkdir -p /var/log/data-ingestion /var/lib/data-ingestion && \
    chown -R ingester:ingester /app /var/log/data-ingestion /var/lib/data-ingestion

# Switch to non-root user
USER ingester

# Environment variables (defaults)
ENV PYTHONUNBUFFERED=1
ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONPATH=/app
ENV LOG_LEVEL=INFO
ENV UPDATE_INTERVAL=60
ENV BATCH_SIZE=100

# Expose ports (matching simple setup)
EXPOSE 8001 9090

# Health check - but use port 8001 like simple setup
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD python -c "import requests; requests.get('http://localhost:8001/health', timeout=5).raise_for_status()" || exit 1

# Use wrapper script to handle symbols from environment
ENTRYPOINT ["/usr/local/bin/start-data-ingestion.sh"]
CMD []