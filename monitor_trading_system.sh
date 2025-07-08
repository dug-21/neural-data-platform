#!/bin/bash

# Comprehensive monitoring script for Neural Trader

echo "📊 Neural Trader System Monitor"
echo "==============================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Check Neural Trader process
echo "🤖 Neural Trader Status:"
if pgrep -f "neural-trader" > /dev/null; then
    PID=$(pgrep -f "neural-trader")
    echo -e "  ${GREEN}✓ Running${NC} (PID: $PID)"
    echo "  Memory: $(ps -o rss= -p $PID | awk '{print int($1/1024) "MB"}')"
    echo "  CPU: $(ps -o %cpu= -p $PID)%"
else
    echo -e "  ${RED}✗ Not running${NC}"
fi
echo ""

# Check database status
echo "🗄️  Database Status:"
if docker exec neural_trader_stocks-timescaledb-1 pg_isready -U neural_trader >/dev/null 2>&1; then
    echo -e "  ${GREEN}✓ PostgreSQL is running${NC}"
    
    # Check if we have data
    MARKET_DATA_COUNT=$(docker exec neural_trader_stocks-timescaledb-1 psql -U neural_trader -d neural_trader_db -t -c "SELECT COUNT(*) FROM market_data;" 2>/dev/null || echo "0")
    TRADES_COUNT=$(docker exec neural_trader_stocks-timescaledb-1 psql -U neural_trader -d neural_trader_db -t -c "SELECT COUNT(*) FROM trades;" 2>/dev/null || echo "0")
    
    echo "  Market Data Records: $MARKET_DATA_COUNT"
    echo "  Trade Records: $TRADES_COUNT"
else
    echo -e "  ${RED}✗ PostgreSQL is not accessible${NC}"
fi
echo ""

# Check Redis status
echo "💾 Redis Status:"
if docker exec neural_trader_stocks-redis-1 redis-cli ping >/dev/null 2>&1; then
    echo -e "  ${GREEN}✓ Redis is running${NC}"
    
    # Get Redis stats
    KEYS=$(docker exec neural_trader_stocks-redis-1 redis-cli -a ${REDIS_PASSWORD} DBSIZE 2>/dev/null | grep -oE '[0-9]+' || echo "0")
    echo "  Keys in cache: $KEYS"
else
    echo -e "  ${RED}✗ Redis is not accessible${NC}"
fi
echo ""

# Check API endpoints
echo "🌐 API Endpoints:"
# Health check
if curl -s -f http://localhost:3030/health >/dev/null 2>&1; then
    echo -e "  ${GREEN}✓ Health endpoint accessible${NC}"
else
    echo -e "  ${YELLOW}⚠ Health endpoint not responding${NC}"
fi

# Metrics
if curl -s -f http://localhost:3031/metrics >/dev/null 2>&1; then
    echo -e "  ${GREEN}✓ Metrics endpoint accessible${NC}"
else
    echo -e "  ${YELLOW}⚠ Metrics endpoint not responding${NC}"
fi
echo ""

# Check logs for errors
echo "📝 Recent Log Activity:"
if [ -f logs/neural-trader.log ]; then
    ERROR_COUNT=$(grep -c "ERROR" logs/neural-trader.log 2>/dev/null || echo "0")
    WARN_COUNT=$(grep -c "WARN" logs/neural-trader.log 2>/dev/null || echo "0")
    echo "  Errors: $ERROR_COUNT"
    echo "  Warnings: $WARN_COUNT"
    echo ""
    echo "  Last 5 log entries:"
    tail -5 logs/neural-trader.log | sed 's/^/  /'
else
    echo "  No log file found"
fi
echo ""

# Market hours check
echo "🕐 Market Status:"
CURRENT_HOUR=$(date +%H)
CURRENT_DAY=$(date +%u)
if [ $CURRENT_DAY -ge 1 ] && [ $CURRENT_DAY -le 5 ]; then
    if [ $CURRENT_HOUR -ge 9 ] && [ $CURRENT_HOUR -lt 16 ]; then
        echo -e "  ${GREEN}Market is OPEN${NC} (US Eastern Time)"
    else
        echo -e "  ${YELLOW}Market is CLOSED${NC} (After hours)"
    fi
else
    echo -e "  ${YELLOW}Market is CLOSED${NC} (Weekend)"
fi
echo ""

# Quick actions
echo "🎯 Quick Actions:"
echo "  1. View live logs:        tail -f logs/neural-trader.log"
echo "  2. Check database:        docker exec -it neural_trader_stocks-timescaledb-1 psql -U neural_trader"
echo "  3. Monitor Redis:         docker exec -it neural_trader_stocks-redis-1 redis-cli -a \${REDIS_PASSWORD} MONITOR"
echo "  4. Restart app:           kill $(pgrep -f neural-trader 2>/dev/null || echo "N/A") && ./start_and_monitor.sh"
echo "  5. View market data:      curl http://localhost:3030/api/v1/market/quotes"
echo ""