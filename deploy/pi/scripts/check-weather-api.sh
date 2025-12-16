#!/bin/bash
# Test OpenWeatherMap API connectivity

if [ -z "$OPENWEATHERMAP_API_KEY" ]; then
    echo "Error: OPENWEATHERMAP_API_KEY not set"
    exit 1
fi

LAT=${LAT:-51.5074}
LON=${LON:--0.1278}

echo "Testing OpenWeatherMap API..."

# Test Current Weather API
weather_response=$(curl -s "https://api.openweathermap.org/data/2.5/weather?lat=${LAT}&lon=${LON}&appid=${OPENWEATHERMAP_API_KEY}")
if echo "$weather_response" | grep -q '"cod":200'; then
    echo "✓ Current Weather API: OK"
else
    echo "✗ Current Weather API: Failed"
    echo "$weather_response"
    exit 1
fi

# Test Air Pollution API
pollution_response=$(curl -s "https://api.openweathermap.org/data/2.5/air_pollution?lat=${LAT}&lon=${LON}&appid=${OPENWEATHERMAP_API_KEY}")
if echo "$pollution_response" | grep -q '"list"'; then
    echo "✓ Air Pollution API: OK"
else
    echo "✗ Air Pollution API: Failed"
    echo "$pollution_response"
    exit 1
fi

echo ""
echo "All API tests passed!"
