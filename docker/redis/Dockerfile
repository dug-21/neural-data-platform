# Custom Redis image optimized for real-time trading data
FROM redis:7-alpine

# Install basic dependencies
RUN apk add --no-cache \
    bash \
    curl

# Copy custom Redis configuration
COPY docker/redis/redis.conf /usr/local/etc/redis/redis.conf

# Copy startup script
COPY docker/redis/docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Create data directory
RUN mkdir -p /data && chown redis:redis /data

# Volume for data persistence
VOLUME ["/data"]

# Expose Redis ports
EXPOSE 6379

# Health check
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD redis-cli ping | grep -q PONG

USER redis

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["redis-server", "/usr/local/etc/redis/redis.conf"]