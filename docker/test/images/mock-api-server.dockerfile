# Mock API server for external service endpoints
FROM python:3.11-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash mockserver

# Set working directory
WORKDIR /app

# Install required Python packages
RUN pip install --no-cache-dir \
    fastapi \
    uvicorn \
    requests \
    faker \
    python-multipart \
    aiofiles

# Copy mock server implementation
COPY docker/test/scripts/mock_api_server.py ./mock_api_server.py
COPY docker/test/scripts/mock_endpoints.py ./mock_endpoints.py
RUN chmod +x *.py

# Create necessary directories
RUN mkdir -p /mock-responses /app/static && \
    chown -R mockserver:mockserver /app /mock-responses

# Switch to non-root user
USER mockserver

# Environment variables
ENV PYTHONUNBUFFERED=1
ENV PYTHONDONTWRITEBYTECODE=1
ENV LOG_LEVEL=INFO
ENV HOST=0.0.0.0
ENV PORT=8000

# Expose port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1

# Default command
CMD ["python", "-m", "uvicorn", "mock_api_server:app", "--host", "0.0.0.0", "--port", "8000", "--log-level", "info"]