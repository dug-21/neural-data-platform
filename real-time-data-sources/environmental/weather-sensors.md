# Weather & Environmental Real-Time Data Sources

## NOAA / National Weather Service ⭐⭐⭐⭐⭐
**The Gold Standard for US Weather Data - Completely Free**

### Overview
- **API Base URL**: `https://api.weather.gov/`
- **Documentation**: https://www.weather.gov/documentation/services-web-api
- **Update Frequency**: 5-60 minutes depending on product
- **Coverage**: United States + Territories

### Key Features
- **No API key required**
- **No rate limits** (reasonable use)
- **Official government data**
- All NWS products available
- Forecasts, observations, alerts

### API Endpoints

#### Current Observations
```bash
# Get observation stations near a point
curl "https://api.weather.gov/points/39.7456,-97.0892"

# Get latest observation from a station
curl "https://api.weather.gov/stations/KLAX/observations/latest"

# Get historical observations
curl "https://api.weather.gov/stations/KLAX/observations?start=2024-01-01T00:00:00Z"
```

#### Forecasts
```javascript
async function getForecast(lat, lon) {
    // First, get the forecast office and grid coordinates
    const pointResponse = await fetch(`https://api.weather.gov/points/${lat},${lon}`);
    const pointData = await pointResponse.json();
    
    // Get the forecast
    const forecastResponse = await fetch(pointData.properties.forecast);
    const forecast = await forecastResponse.json();
    
    return forecast.properties.periods;
}

// Example usage
const forecast = await getForecast(40.7128, -74.0060);  // NYC
forecast.forEach(period => {
    console.log(`${period.name}: ${period.temperature}°${period.temperatureUnit} - ${period.shortForecast}`);
});
```

#### Real-Time Alerts
```javascript
// Active alerts for a state
const alertsResponse = await fetch('https://api.weather.gov/alerts/active?area=CA');
const alerts = await alertsResponse.json();

// WebSocket-like polling for alerts
class NOAAAlertMonitor {
    constructor(states = ['CA', 'TX', 'FL']) {
        this.states = states;
        this.knownAlerts = new Set();
        this.pollInterval = 60000; // 1 minute
    }
    
    async checkAlerts() {
        for (const state of this.states) {
            const response = await fetch(`https://api.weather.gov/alerts/active?area=${state}`);
            const data = await response.json();
            
            data.features.forEach(alert => {
                const id = alert.properties.id;
                if (!this.knownAlerts.has(id)) {
                    this.knownAlerts.add(id);
                    this.onNewAlert(alert);
                }
            });
        }
    }
    
    onNewAlert(alert) {
        const props = alert.properties;
        console.log(`NEW ALERT: ${props.event} - ${props.headline}`);
        console.log(`Areas: ${props.areaDesc}`);
        console.log(`Severity: ${props.severity}`);
    }
    
    start() {
        this.checkAlerts();
        setInterval(() => this.checkAlerts(), this.pollInterval);
    }
}
```

### Radar Data
```javascript
// Get radar stations
const radarStations = await fetch('https://api.weather.gov/radar/stations');

// Radar images are available via separate services
// Example: https://radar.weather.gov/ridge/standard/KLOT_loop.gif
```

---

## OpenWeatherMap ⭐⭐⭐⭐
**Best Global Coverage with Free Tier**

### Overview
- **API Base**: `https://api.openweathermap.org/data/2.5/`
- **Documentation**: https://openweathermap.org/api
- **Update Frequency**: 10 minutes
- **Coverage**: Global

### Free Tier Details
- 1,000 API calls/day
- 60 calls/minute
- Current weather
- 5-day forecast
- Air pollution data
- Weather maps

### API Examples

#### Current Weather
```python
import requests

API_KEY = 'your_api_key'
BASE_URL = 'https://api.openweathermap.org/data/2.5'

# Current weather by city
city = 'London'
response = requests.get(f'{BASE_URL}/weather?q={city}&appid={API_KEY}&units=metric')
data = response.json()

print(f"Temperature: {data['main']['temp']}°C")
print(f"Feels like: {data['main']['feels_like']}°C")
print(f"Weather: {data['weather'][0]['description']}")

# By coordinates
lat, lon = 51.5074, -0.1278
response = requests.get(f'{BASE_URL}/weather?lat={lat}&lon={lon}&appid={API_KEY}')
```

#### One Call API (Comprehensive Data)
```javascript
// One Call API 3.0 - requires subscription but has free tier
const lat = 33.44;
const lon = -94.04;
const url = `https://api.openweathermap.org/data/3.0/onecall?lat=${lat}&lon=${lon}&appid=${API_KEY}`;

fetch(url)
    .then(response => response.json())
    .then(data => {
        console.log('Current:', data.current);
        console.log('Minutely precipitation:', data.minutely);
        console.log('Hourly forecast:', data.hourly);
        console.log('Daily forecast:', data.daily);
        console.log('Weather alerts:', data.alerts);
    });
```

#### Bulk Download
```python
# Download multiple cities at once (up to 20)
cities = ['London,uk', 'Paris,fr', 'New York,us', 'Tokyo,jp']
city_ids = ','.join([str(city) for city in cities])

url = f'https://api.openweathermap.org/data/2.5/group?id={city_ids}&appid={API_KEY}'
```

---

## PurpleAir Air Quality Network ⭐⭐⭐⭐
**Real-Time Air Quality from 10,000+ Sensors**

### Overview
- **API URL**: `https://api.purpleair.com/v1/`
- **Documentation**: https://api.purpleair.com/
- **Update Frequency**: 2 minutes
- **Coverage**: Global (community-driven)

### Free Tier Details
- 1 million data points/day
- Real-time sensor data
- Historical data access
- No commercial use without license

### API Implementation

#### Authentication
```python
import requests

headers = {
    'X-API-Key': 'YOUR_API_KEY'
}
```

#### Get Sensors in Area
```python
# Get sensors within a bounding box
nwlat, nwlng = 37.8044, -122.4194  # Northwest corner
selat, selng = 37.7034, -122.3816  # Southeast corner

url = f'https://api.purpleair.com/v1/sensors?fields=name,latitude,longitude,pm2.5&location_type=0&nwlat={nwlat}&nwlng={nwlng}&selat={selat}&selng={selng}'

response = requests.get(url, headers=headers)
sensors = response.json()['data']
```

#### Real-Time Monitoring
```javascript
class PurpleAirMonitor {
    constructor(apiKey, sensorIds) {
        this.apiKey = apiKey;
        this.sensorIds = sensorIds;
        this.updateInterval = 120000; // 2 minutes
    }
    
    async getSensorData(sensorId) {
        const url = `https://api.purpleair.com/v1/sensors/${sensorId}?fields=pm2.5,temperature,humidity,pressure`;
        const response = await fetch(url, {
            headers: { 'X-API-Key': this.apiKey }
        });
        return response.json();
    }
    
    async updateAll() {
        const updates = await Promise.all(
            this.sensorIds.map(id => this.getSensorData(id))
        );
        
        updates.forEach((data, index) => {
            const sensor = data.sensor;
            console.log(`Sensor ${this.sensorIds[index]}:`);
            console.log(`  PM2.5: ${sensor.pm2.5} μg/m³`);
            console.log(`  Temp: ${sensor.temperature}°F`);
            console.log(`  AQI: ${this.calculateAQI(sensor.pm2.5)}`);
        });
    }
    
    calculateAQI(pm25) {
        // Simplified AQI calculation
        if (pm25 <= 12) return Math.round((50/12) * pm25);
        if (pm25 <= 35.4) return Math.round((100-51)/(35.4-12.1) * (pm25-12.1) + 51);
        if (pm25 <= 55.4) return Math.round((150-101)/(55.4-35.5) * (pm25-35.5) + 101);
        // ... additional ranges
        return 200; // Simplified
    }
    
    start() {
        this.updateAll();
        setInterval(() => this.updateAll(), this.updateInterval);
    }
}
```

---

## Weather Underground PWS Network ⭐⭐⭐
**Personal Weather Station Data**

### Overview
- **API**: Via Weather.com/IBM
- **Update Frequency**: 2.5-5 minutes
- **Coverage**: 250,000+ stations globally

### Access Methods

#### Direct PWS Data
```python
# Note: Requires API key from IBM Weather
import requests

API_KEY = 'your_ibm_api_key'
station_id = 'KCASANFR131'

url = f'https://api.weather.com/v2/pws/observations/current?stationId={station_id}&format=json&units=m&apikey={API_KEY}'
response = requests.get(url)
data = response.json()
```

#### Alternative: Scraping (Use Carefully)
```python
# Direct station URL (no API needed but respect robots.txt)
station_url = f'https://www.wunderground.com/dashboard/pws/{station_id}'
# Parse HTML or use their internal API endpoints
```

---

## EPA AirNow ⭐⭐⭐⭐
**Official US Air Quality Data**

### Overview
- **API URL**: `https://www.airnowapi.org/`
- **Documentation**: https://docs.airnowapi.org/
- **Update Frequency**: Hourly
- **Coverage**: United States

### API Access
```javascript
const API_KEY = 'your_api_key';

// Current conditions by ZIP
const zipCode = '20002';
const url = `https://www.airnowapi.org/aq/observation/zipCode/current/?format=application/json&zipCode=${zipCode}&API_KEY=${API_KEY}`;

// Forecast
const forecastUrl = `https://www.airnowapi.org/aq/forecast/zipCode/?format=application/json&zipCode=${zipCode}&API_KEY=${API_KEY}`;

// Historical data
const date = '2024-01-15';
const histUrl = `https://www.airnowapi.org/aq/observation/zipCode/historical/?format=application/json&zipCode=${zipCode}&date=${date}&API_KEY=${API_KEY}`;
```

---

## USGS Earthquake Data ⭐⭐⭐⭐⭐
**Real-Time Seismic Activity**

### Overview
- **API URL**: `https://earthquake.usgs.gov/fdsnws/event/1/`
- **GeoJSON Feeds**: https://earthquake.usgs.gov/earthquakes/feed/
- **Update Frequency**: Near real-time (minutes)
- **Coverage**: Global

### Real-Time Feeds
```javascript
// Real-time earthquake feeds (no API key needed)
const feeds = {
    // All earthquakes
    all_hour: 'https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_hour.geojson',
    all_day: 'https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_day.geojson',
    all_week: 'https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_week.geojson',
    
    // Significant earthquakes
    significant_hour: 'https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/significant_hour.geojson',
    significant_day: 'https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/significant_day.geojson',
    
    // By magnitude
    m4_5_hour: 'https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/4.5_hour.geojson',
    m2_5_hour: 'https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/2.5_hour.geojson',
    m1_0_hour: 'https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/1.0_hour.geojson'
};

// Monitor for new earthquakes
class EarthquakeMonitor {
    constructor(minimumMagnitude = 4.5) {
        this.minMag = minimumMagnitude;
        this.knownQuakes = new Set();
        this.checkInterval = 60000; // 1 minute
    }
    
    async checkQuakes() {
        const response = await fetch(feeds.all_hour);
        const data = await response.json();
        
        data.features.forEach(quake => {
            const props = quake.properties;
            const id = quake.id;
            
            if (props.mag >= this.minMag && !this.knownQuakes.has(id)) {
                this.knownQuakes.add(id);
                this.onNewQuake(quake);
            }
        });
    }
    
    onNewQuake(quake) {
        const props = quake.properties;
        const coords = quake.geometry.coordinates;
        
        console.log(`🚨 EARTHQUAKE DETECTED!`);
        console.log(`Magnitude: ${props.mag}`);
        console.log(`Location: ${props.place}`);
        console.log(`Depth: ${coords[2]} km`);
        console.log(`Time: ${new Date(props.time).toISOString()}`);
        console.log(`URL: ${props.url}`);
    }
    
    start() {
        this.checkQuakes();
        setInterval(() => this.checkQuakes(), this.checkInterval);
    }
}
```

---

## Ocean & Marine Data - NDBC Buoys ⭐⭐⭐⭐⭐
**NOAA National Data Buoy Center**

### Overview
- **Base URL**: `https://www.ndbc.noaa.gov/`
- **Update Frequency**: 10 minutes to 1 hour
- **Coverage**: Oceans worldwide
- **Data**: Wave height, wind, pressure, temperature

### Real-Time Data Access
```python
import requests
import pandas as pd

# Get latest observations from a buoy
buoy_id = '46042'  # Monterey Bay, CA
url = f'https://www.ndbc.noaa.gov/data/realtime2/{buoy_id}.txt'

# Parse the data
response = requests.get(url)
lines = response.text.strip().split('\n')

# First line is header
headers = lines[0].split()

# Parse recent observations
data = []
for line in lines[2:10]:  # Skip units line, get recent obs
    values = line.split()
    data.append(values)

df = pd.DataFrame(data, columns=headers)

# Latest observation
latest = df.iloc[0]
print(f"Wave Height: {latest['WVHT']} m")
print(f"Wind Speed: {latest['WSPD']} m/s")
print(f"Air Temp: {latest['ATMP']} °C")
```

### Bulk Data Access
```javascript
// RSS feed for buoy observations
const buoyRSS = 'https://www.ndbc.noaa.gov/data/latest_obs/latest_obs.txt';

// Parse all buoys at once
async function getAllBuoys() {
    const response = await fetch(buoyRSS);
    const text = await response.text();
    
    const lines = text.split('\n');
    const headers = lines[0].trim().split(/\s+/);
    
    const buoys = [];
    for (let i = 2; i < lines.length; i++) {
        const values = lines[i].trim().split(/\s+/);
        if (values.length > 1) {
            const buoy = {};
            headers.forEach((header, index) => {
                buoy[header] = values[index];
            });
            buoys.push(buoy);
        }
    }
    
    return buoys;
}
```

---

## WeatherAPI.com ⭐⭐⭐⭐
**Alternative Global Weather Provider**

### Overview
- **API Base**: `https://api.weatherapi.com/v1/`
- **Documentation**: https://www.weatherapi.com/docs/
- **Update Frequency**: Real-time
- **Coverage**: Global

### Free Tier
- 1 million calls/month
- Real-time weather
- 3-day forecast
- Astronomy data
- Sports data
- Time zone API

### Implementation
```python
import aiohttp
import asyncio

class WeatherAPIClient:
    def __init__(self, api_key):
        self.api_key = api_key
        self.base_url = 'https://api.weatherapi.com/v1'
    
    async def get_current(self, location):
        async with aiohttp.ClientSession() as session:
            url = f'{self.base_url}/current.json'
            params = {
                'key': self.api_key,
                'q': location,
                'aqi': 'yes'  # Include air quality
            }
            async with session.get(url, params=params) as response:
                return await response.json()
    
    async def get_forecast(self, location, days=3):
        async with aiohttp.ClientSession() as session:
            url = f'{self.base_url}/forecast.json'
            params = {
                'key': self.api_key,
                'q': location,
                'days': days,
                'aqi': 'yes',
                'alerts': 'yes'
            }
            async with session.get(url, params=params) as response:
                return await response.json()
    
    async def get_bulk_weather(self, locations):
        tasks = [self.get_current(loc) for loc in locations]
        return await asyncio.gather(*tasks)

# Usage
client = WeatherAPIClient('your_api_key')
weather = await client.get_current('London')
print(f"Temperature: {weather['current']['temp_c']}°C")
print(f"Air Quality Index: {weather['current']['air_quality']['pm2_5']}")
```

---

## Environmental Monitoring Networks

### RadNet - EPA Radiation Monitoring
```python
# Near real-time radiation data
# Available through EPA's RadNet API
radnet_url = 'https://www.epa.gov/radnet/radnet-csv-files'
```

### USGS Water Data
```javascript
// Real-time water levels, flow, quality
const siteCode = '01646500';  // Potomac River
const url = `https://waterservices.usgs.gov/nwis/iv/?sites=${siteCode}&format=json`;

fetch(url)
    .then(response => response.json())
    .then(data => {
        const timeSeries = data.value.timeSeries;
        timeSeries.forEach(series => {
            console.log(`${series.variable.variableDescription}: ${series.values[0].value[0].value}`);
        });
    });
```

---

## Integration Best Practices

### 1. Unified Weather Interface
```javascript
class UnifiedWeatherProvider {
    constructor() {
        this.providers = {
            noaa: new NOAAProvider(),
            openweather: new OpenWeatherProvider(API_KEY),
            weatherapi: new WeatherAPIProvider(API_KEY)
        };
    }
    
    async getWeather(lat, lon) {
        // Try providers in order of preference
        const providers = ['noaa', 'openweather', 'weatherapi'];
        
        for (const provider of providers) {
            try {
                return await this.providers[provider].getWeather(lat, lon);
            } catch (error) {
                console.error(`${provider} failed:`, error);
                continue;
            }
        }
        
        throw new Error('All weather providers failed');
    }
}
```

### 2. Data Caching Strategy
```python
import time
from functools import lru_cache
import hashlib

class WeatherCache:
    def __init__(self, ttl=600):  # 10 minute cache
        self.ttl = ttl
        self.cache = {}
    
    def get_cache_key(self, provider, lat, lon):
        # Round coordinates to reduce cache misses
        lat_rounded = round(lat, 2)
        lon_rounded = round(lon, 2)
        return f"{provider}:{lat_rounded}:{lon_rounded}"
    
    def get(self, provider, lat, lon):
        key = self.get_cache_key(provider, lat, lon)
        if key in self.cache:
            data, timestamp = self.cache[key]
            if time.time() - timestamp < self.ttl:
                return data
        return None
    
    def set(self, provider, lat, lon, data):
        key = self.get_cache_key(provider, lat, lon)
        self.cache[key] = (data, time.time())
```

### 3. Rate Limit Management
```javascript
class RateLimiter {
    constructor(callsPerMinute) {
        this.callsPerMinute = callsPerMinute;
        this.calls = [];
    }
    
    async throttle() {
        const now = Date.now();
        const minuteAgo = now - 60000;
        
        // Remove old calls
        this.calls = this.calls.filter(time => time > minuteAgo);
        
        if (this.calls.length >= this.callsPerMinute) {
            const oldestCall = this.calls[0];
            const waitTime = 60000 - (now - oldestCall) + 100;
            await new Promise(resolve => setTimeout(resolve, waitTime));
        }
        
        this.calls.push(now);
    }
}
```

---

## Comparison Matrix

| Provider | Coverage | Update Freq | Free Limit | Best For |
|----------|----------|-------------|------------|----------|
| NOAA/NWS | US only | 5-60 min | Unlimited | US official data |
| OpenWeatherMap | Global | 10 min | 1000/day | Global coverage |
| PurpleAir | Global | 2 min | 1M points/day | Air quality |
| WeatherAPI | Global | Real-time | 1M/month | Alternative to OWM |
| EPA AirNow | US only | Hourly | Reasonable | Official AQ data |
| USGS | Global | Minutes | Unlimited | Earthquakes/water |

## Additional Resources
- **Windy API**: https://api.windy.com/ (webcams, forecasts)
- **Tomorrow.io**: https://www.tomorrow.io/ (formerly ClimaCell)
- **Meteomatics**: https://www.meteomatics.com/ (premium but has trial)
- **DarkSky**: Now part of Apple, API discontinued
- **ECMWF**: https://www.ecmwf.int/ (European model data)