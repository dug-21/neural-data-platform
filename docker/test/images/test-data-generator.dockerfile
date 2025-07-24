# Test data generator for creating realistic test datasets
FROM python:3.11-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    postgresql-client \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash generator

# Set working directory
WORKDIR /app

# Install required Python packages
RUN pip install --no-cache-dir \
    psycopg2-binary \
    pandas \
    numpy \
    faker \
    yfinance \
    requests \
    sqlalchemy \
    asyncpg \
    python-dateutil

# Copy test data generation scripts
COPY docker/test/scripts/generate_test_data.py ./generate_test_data.py
COPY docker/test/scripts/market_data_generator.py ./market_data_generator.py
COPY docker/test/scripts/sentiment_data_generator.py ./sentiment_data_generator.py
RUN chmod +x *.py

# Create necessary directories
RUN mkdir -p /test-fixtures/generated && \
    chown -R generator:generator /app /test-fixtures

# Switch to non-root user
USER generator

# Environment variables
ENV PYTHONUNBUFFERED=1
ENV PYTHONDONTWRITEBYTECODE=1
ENV LOG_LEVEL=INFO

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=2 \
    CMD python -c "import sys; sys.exit(0)"

# Default command to generate test data
CMD ["python", "generate_test_data.py"]