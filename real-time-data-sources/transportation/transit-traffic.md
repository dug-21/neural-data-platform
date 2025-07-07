# Transportation Real-Time Data Sources

## OpenSky Network ⭐⭐⭐⭐⭐
**Free Real-Time Flight Tracking**

### Overview
- **API URL**: `https://opensky-network.org/api/`
- **Documentation**: https://openskynetwork.github.io/opensky-api/
- **Update Frequency**: 5-10 seconds
- **Coverage**: Global ADS-B coverage

### Free Access Details
- Anonymous: 10 API calls per 10 seconds
- Registered: 4000 API calls per day
- Academic/Research: Extended limits
- All aircraft positions free
- Historical data available

### REST API Implementation
```python
import requests
from datetime import datetime

class OpenSkyTracker:
    def __init__(self, username=None, password=None):
        self.base_url = 'https://opensky-network.org/api'
        self.auth = (username, password) if username else None
    
    def get_all_states(self):
        """Get current state of all aircraft"""
        url = f'{self.base_url}/states/all'
        response = requests.get(url, auth=self.auth)
        return response.json()
    
    def get_area_states(self, min_lat, max_lat, min_lon, max_lon):
        """Get aircraft in bounding box"""
        url = f'{self.base_url}/states/all'
        params = {
            'lamin': min_lat,
            'lamax': max_lat,
            'lomin': min_lon,
            'lomax': max_lon
        }
        response = requests.get(url, params=params, auth=self.auth)
        return response.json()
    
    def track_flight(self, icao24):
        """Track specific aircraft by ICAO24 address"""
        url = f'{self.base_url}/states/all'
        params = {'icao24': icao24}
        response = requests.get(url, params=params, auth=self.auth)
        return response.json()

# Usage example
tracker = OpenSkyTracker()

# Get all flights over San Francisco
sf_flights = tracker.get_area_states(
    min_lat=37.5, max_lat=38.0,
    min_lon=-122.7, max_lon=-122.2
)

for flight in sf_flights['states']:
    callsign = flight[1].strip() if flight[1] else 'N/A'
    altitude = flight[7]  # meters
    velocity = flight[9]  # m/s
    print(f"Flight {callsign}: Alt {altitude}m, Speed {velocity}m/s")
```

### Real-Time Monitoring System
```javascript
class FlightMonitor {
    constructor(bounds) {
        this.bounds = bounds;
        this.knownFlights = new Map();
        this.updateInterval = 10000; // 10 seconds
    }
    
    async updateFlights() {
        const url = new URL('https://opensky-network.org/api/states/all');
        url.searchParams.append('lamin', this.bounds.minLat);
        url.searchParams.append('lamax', this.bounds.maxLat);
        url.searchParams.append('lomin', this.bounds.minLon);
        url.searchParams.append('lomax', this.bounds.maxLon);
        
        try {
            const response = await fetch(url);
            const data = await response.json();
            
            this.processFlights(data.states || []);
        } catch (error) {
            console.error('Failed to fetch flights:', error);
        }
    }
    
    processFlights(states) {
        const currentFlights = new Set();
        
        states.forEach(state => {
            const icao24 = state[0];
            const callsign = state[1]?.trim();
            const position = {
                lat: state[6],
                lon: state[5],
                altitude: state[7],
                velocity: state[9],
                heading: state[10]
            };
            
            currentFlights.add(icao24);
            
            if (!this.knownFlights.has(icao24)) {
                this.onNewFlight(icao24, callsign, position);
            } else {
                this.onFlightUpdate(icao24, callsign, position);
            }
            
            this.knownFlights.set(icao24, { callsign, position });
        });
        
        // Check for departed flights
        for (const [icao24, flight] of this.knownFlights) {
            if (!currentFlights.has(icao24)) {
                this.onFlightDeparted(icao24, flight.callsign);
                this.knownFlights.delete(icao24);
            }
        }
    }
    
    onNewFlight(icao24, callsign, position) {
        console.log(`✈️ New flight: ${callsign || icao24} at ${position.altitude}m`);
    }
    
    onFlightUpdate(icao24, callsign, position) {
        // Track movement
    }
    
    onFlightDeparted(icao24, callsign) {
        console.log(`✈️ Flight departed: ${callsign || icao24}`);
    }
    
    start() {
        this.updateFlights();
        setInterval(() => this.updateFlights(), this.updateInterval);
    }
}

// Monitor flights over major airport
const jfkMonitor = new FlightMonitor({
    minLat: 40.5, maxLat: 40.8,
    minLon: -73.9, maxLon: -73.6
});
jfkMonitor.start();
```

---

## GTFS Realtime Feeds ⭐⭐⭐⭐⭐
**Standard Transit Data Format**

### Overview
- **Specification**: https://gtfs.org/realtime/
- **Format**: Protocol Buffers
- **Update Frequency**: 10-30 seconds
- **Coverage**: 1000+ transit agencies worldwide

### Major City Feeds

#### New York MTA
```python
from google.transit import gtfs_realtime_pb2
import requests
import time

class MTATracker:
    def __init__(self, api_key):
        self.api_key = api_key
        self.feeds = {
            '123456S': 'https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs',
            'ACE': 'https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-ace',
            'BDFM': 'https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-bdfm',
            'G': 'https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-g',
            'JZ': 'https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-jz',
            'L': 'https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-l',
            'NQR': 'https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-nqrw',
            '7': 'https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-7'
        }
    
    def get_feed(self, line):
        """Get real-time feed for subway line"""
        headers = {'x-api-key': self.api_key}
        response = requests.get(self.feeds[line], headers=headers)
        
        feed = gtfs_realtime_pb2.FeedMessage()
        feed.ParseFromString(response.content)
        return feed
    
    def get_train_positions(self, line):
        """Get current train positions"""
        feed = self.get_feed(line)
        trains = []
        
        for entity in feed.entity:
            if entity.HasField('vehicle'):
                vehicle = entity.vehicle
                trains.append({
                    'train_id': vehicle.trip.trip_id,
                    'route': vehicle.trip.route_id,
                    'stop_id': vehicle.stop_id,
                    'status': vehicle.current_status,
                    'timestamp': vehicle.timestamp
                })
        
        return trains
    
    def get_trip_updates(self, line, stop_id):
        """Get arrival predictions for a stop"""
        feed = self.get_feed(line)
        arrivals = []
        
        for entity in feed.entity:
            if entity.HasField('trip_update'):
                trip_update = entity.trip_update
                for stop_time_update in trip_update.stop_time_update:
                    if stop_time_update.stop_id == stop_id:
                        arrivals.append({
                            'route': trip_update.trip.route_id,
                            'arrival_time': stop_time_update.arrival.time,
                            'delay': stop_time_update.arrival.delay
                        })
        
        return sorted(arrivals, key=lambda x: x['arrival_time'])
```

#### San Francisco Muni
```javascript
// SF Muni real-time predictions
class MuniTracker {
    constructor() {
        this.baseUrl = 'https://api.511.org/transit';
        this.apiKey = 'YOUR_511_API_KEY';
    }
    
    async getVehiclePositions(route) {
        const url = `${this.baseUrl}/VehicleMonitoring`;
        const params = new URLSearchParams({
            api_key: this.apiKey,
            agency: 'SF',
            format: 'json'
        });
        
        const response = await fetch(`${url}?${params}`);
        const data = await response.json();
        
        return data.Siri.ServiceDelivery.VehicleMonitoringDelivery.VehicleActivity
            .filter(v => v.MonitoredVehicleJourney.LineRef === route);
    }
    
    async getStopPredictions(stopCode) {
        const url = `${this.baseUrl}/StopMonitoring`;
        const params = new URLSearchParams({
            api_key: this.apiKey,
            agency: 'SF',
            stopCode: stopCode,
            format: 'json'
        });
        
        const response = await fetch(`${url}?${params}`);
        const data = await response.json();
        
        return data.Siri.ServiceDelivery.StopMonitoringDelivery
            .MonitoredStopVisit.map(visit => ({
                line: visit.MonitoredVehicleJourney.LineRef,
                destination: visit.MonitoredVehicleJourney.DestinationName,
                expectedArrival: visit.MonitoredVehicleJourney.MonitoredCall.ExpectedArrivalTime,
                vehicleLocation: visit.MonitoredVehicleJourney.VehicleLocation
            }));
    }
}
```

#### London TfL
```python
# Transport for London Unified API
class TfLTracker:
    def __init__(self, api_key=None):
        self.base_url = 'https://api.tfl.gov.uk'
        self.api_key = api_key
    
    def get_arrivals(self, stop_id):
        """Get live arrivals at a stop"""
        url = f'{self.base_url}/StopPoint/{stop_id}/Arrivals'
        params = {'app_key': self.api_key} if self.api_key else {}
        
        response = requests.get(url, params=params)
        arrivals = response.json()
        
        return sorted(arrivals, key=lambda x: x['timeToStation'])
    
    def get_line_status(self):
        """Get status of all lines"""
        url = f'{self.base_url}/Line/Mode/tube,dlr,overground/Status'
        response = requests.get(url)
        return response.json()
    
    def get_disruptions(self):
        """Get current disruptions"""
        url = f'{self.base_url}/Disruptions/Mode/tube,bus,dlr'
        response = requests.get(url)
        return response.json()
```

---

## Marine/Ship Tracking - AIS Data ⭐⭐⭐⭐

### Norwegian Coastal Administration (Free)
```python
import asyncio
import websockets
import json

class AISTracker:
    def __init__(self):
        self.ws_url = 'wss://ais.kystverket.no/ws'
        self.ships = {}
    
    async def connect(self):
        async with websockets.connect(self.ws_url) as websocket:
            # Subscribe to area
            subscribe_msg = {
                "type": "subscribe",
                "area": {
                    "north": 71.0,
                    "south": 57.0,
                    "east": 31.0,
                    "west": 4.0
                }
            }
            await websocket.send(json.dumps(subscribe_msg))
            
            # Receive updates
            async for message in websocket:
                data = json.loads(message)
                self.process_ais_message(data)
    
    def process_ais_message(self, data):
        if data['type'] == 'position':
            mmsi = data['mmsi']
            self.ships[mmsi] = {
                'name': data.get('name', 'Unknown'),
                'position': {
                    'lat': data['lat'],
                    'lon': data['lon']
                },
                'speed': data.get('speed', 0),
                'course': data.get('course', 0),
                'timestamp': data['timestamp']
            }
            print(f"Ship {data.get('name', mmsi)}: {data['lat']}, {data['lon']}")
```

### MarineTraffic (Limited Free)
```javascript
// Note: MarineTraffic API requires paid subscription
// This shows structure for those with access

class MarineTrafficAPI {
    constructor(apiKey) {
        this.apiKey = apiKey;
        this.baseUrl = 'https://services.marinetraffic.com/api';
    }
    
    async getVesselPositions(params = {}) {
        const endpoint = '/exportvessel/latest';
        const queryParams = new URLSearchParams({
            v: '5',
            apikey: this.apiKey,
            timespan: params.timespan || '20',
            ...params
        });
        
        const response = await fetch(`${this.baseUrl}${endpoint}?${queryParams}`);
        return response.json();
    }
    
    async trackVessel(mmsi) {
        return this.getVesselPositions({ mmsi });
    }
}
```

---

## Traffic Flow Data ⭐⭐⭐

### HERE Traffic API (Freemium)
```javascript
class HERETrafficFlow {
    constructor(apiKey) {
        this.apiKey = apiKey;
        this.baseUrl = 'https://traffic.ls.hereapi.com/traffic/6.3';
    }
    
    async getFlowData(bbox) {
        const url = `${this.baseUrl}/flow.json`;
        const params = {
            apiKey: this.apiKey,
            bbox: bbox, // format: "lat1,lon1;lat2,lon2"
            responseattributes: 'sh,fc'
        };
        
        const response = await fetch(`${url}?${new URLSearchParams(params)}`);
        return response.json();
    }
    
    async getIncidents(bbox) {
        const url = `${this.baseUrl}/incidents.json`;
        const params = {
            apiKey: this.apiKey,
            bbox: bbox,
            criticality: 'major,critical'
        };
        
        const response = await fetch(`${url}?${new URLSearchParams(params)}`);
        return response.json();
    }
}
```

### TomTom Traffic (Limited Free)
```python
class TomTomTraffic:
    def __init__(self, api_key):
        self.api_key = api_key
        self.base_url = 'https://api.tomtom.com/traffic'
    
    def get_flow_segment(self, point, zoom=10):
        """Get traffic flow for road segment"""
        lat, lon = point
        url = f'{self.base_url}/services/4/flowSegmentData/absolute/{zoom}/json'
        params = {
            'key': self.api_key,
            'point': f'{lat},{lon}'
        }
        
        response = requests.get(url, params=params)
        return response.json()
    
    def get_incidents(self, bbox):
        """Get traffic incidents in area"""
        url = f'{self.base_url}/services/5/incidentDetails'
        params = {
            'key': self.api_key,
            'bbox': bbox,
            'fields': '{incidents{type,geometry,properties}}'
        }
        
        response = requests.get(url, params=params)
        return response.json()
```

### 511.org (California) ⭐⭐⭐⭐
```javascript
// Free traffic data for California
class California511 {
    constructor(apiKey) {
        this.apiKey = apiKey;
        this.baseUrl = 'https://api.511.org';
    }
    
    async getTrafficEvents() {
        const url = `${this.baseUrl}/traffic/events`;
        const response = await fetch(`${url}?api_key=${this.apiKey}&format=json`);
        return response.json();
    }
    
    async getCHPIncidents() {
        const url = `${this.baseUrl}/traffic/chp/incidents`;
        const response = await fetch(`${url}?api_key=${this.apiKey}&format=json`);
        return response.json();
    }
    
    async getRoadConditions(roadway) {
        const url = `${this.baseUrl}/traffic/roadconditions`;
        const params = new URLSearchParams({
            api_key: this.apiKey,
            roadway: roadway,
            format: 'json'
        });
        
        const response = await fetch(`${url}?${params}`);
        return response.json();
    }
}
```

---

## Bike Share Systems (GBFS) ⭐⭐⭐⭐⭐

### General Bikeshare Feed Specification
```python
import aiohttp
import asyncio

class GBFSClient:
    """Universal bike share data client"""
    
    # Known GBFS feeds
    FEEDS = {
        'citibike_nyc': 'https://gbfs.citibikenyc.com/gbfs/gbfs.json',
        'divvy_chicago': 'https://gbfs.divvybikes.com/gbfs/gbfs.json',
        'capital_bikeshare': 'https://gbfs.capitalbikeshare.com/gbfs/gbfs.json',
        'bay_wheels': 'https://gbfs.baywheels.com/gbfs/gbfs.json',
        'bluebikes_boston': 'https://gbfs.bluebikes.com/gbfs/gbfs.json'
    }
    
    def __init__(self, system='citibike_nyc'):
        self.system = system
        self.base_url = self.FEEDS[system]
        self.endpoints = {}
    
    async def discover_feeds(self):
        """Get all available endpoints"""
        async with aiohttp.ClientSession() as session:
            async with session.get(self.base_url) as response:
                data = await response.json()
                
                for feed in data['data']['en']['feeds']:
                    self.endpoints[feed['name']] = feed['url']
    
    async def get_station_status(self):
        """Get real-time station status"""
        if 'station_status' not in self.endpoints:
            await self.discover_feeds()
        
        async with aiohttp.ClientSession() as session:
            async with session.get(self.endpoints['station_status']) as response:
                data = await response.json()
                return data['data']['stations']
    
    async def get_station_info(self):
        """Get static station information"""
        if 'station_information' not in self.endpoints:
            await self.discover_feeds()
        
        async with aiohttp.ClientSession() as session:
            async with session.get(self.endpoints['station_information']) as response:
                data = await response.json()
                return data['data']['stations']
    
    async def monitor_stations(self, station_ids, interval=30):
        """Monitor specific stations"""
        while True:
            statuses = await self.get_station_status()
            
            for status in statuses:
                if status['station_id'] in station_ids:
                    print(f"Station {status['station_id']}:")
                    print(f"  Bikes available: {status['num_bikes_available']}")
                    print(f"  Docks available: {status['num_docks_available']}")
            
            await asyncio.sleep(interval)

# Usage
async def main():
    client = GBFSClient('citibike_nyc')
    await client.discover_feeds()
    
    # Get all station status
    stations = await self.get_station_status()
    
    # Find low bike stations
    low_bike_stations = [s for s in stations if s['num_bikes_available'] < 5]
    print(f"Stations with < 5 bikes: {len(low_bike_stations)}")

asyncio.run(main())
```

---

## Rail/Train Data ⭐⭐⭐

### Amtrak (US)
```python
# Amtrak real-time train tracking
class AmtrakTracker:
    def __init__(self):
        self.base_url = 'https://maps.amtrak.com/api/trains'
    
    def get_all_trains(self):
        """Get all active trains"""
        response = requests.get(self.base_url)
        return response.json()
    
    def track_train(self, train_number):
        """Track specific train"""
        trains = self.get_all_trains()
        
        for train in trains:
            if train['trainNumber'] == train_number:
                return {
                    'number': train['trainNumber'],
                    'route': train['routeName'],
                    'position': {
                        'lat': train['lat'],
                        'lon': train['lon']
                    },
                    'speed': train.get('speed', 0),
                    'heading': train.get('heading', 0),
                    'delay': train.get('delay', 0),
                    'next_stop': train.get('nextStop')
                }
        
        return None
```

### UK Rail (Network Rail)
```javascript
// UK rail data via Network Rail Data Feeds
// Requires registration at https://datafeeds.networkrail.co.uk/

const StompJs = require('@stomp/stompjs');

class NetworkRailClient {
    constructor(username, password) {
        this.username = username;
        this.password = password;
        this.client = new StompJs.Client({
            brokerURL: 'wss://datafeeds.networkrail.co.uk/stomp',
            connectHeaders: {
                login: username,
                passcode: password
            },
            reconnectDelay: 5000
        });
    }
    
    connect() {
        this.client.onConnect = () => {
            console.log('Connected to Network Rail');
            
            // Subscribe to train movements
            this.client.subscribe('/topic/TRAIN_MVT_ALL_TOC', (message) => {
                const movements = JSON.parse(message.body);
                this.handleTrainMovements(movements);
            });
            
            // Subscribe to delays
            this.client.subscribe('/topic/RTPPM_ALL', (message) => {
                const rtppm = JSON.parse(message.body);
                this.handleDelays(rtppm);
            });
        };
        
        this.client.activate();
    }
    
    handleTrainMovements(movements) {
        movements.forEach(movement => {
            if (movement.header.msg_type === 'activation') {
                console.log(`Train activated: ${movement.body.train_id}`);
            } else if (movement.header.msg_type === 'movement') {
                console.log(`Train ${movement.body.train_id} at ${movement.body.loc_stanox}`);
            }
        });
    }
}
```

---

## Integration Example: Multi-Modal Transport Tracker

```javascript
class MultiModalTransportTracker {
    constructor(config) {
        this.config = config;
        this.trackers = {
            flights: new OpenSkyTracker(),
            transit: new GTFSTracker(config.transitFeeds),
            traffic: new TrafficFlowTracker(config.trafficApis),
            bikes: new GBFSClient(config.bikeSystem),
            ships: new AISTracker()
        };
    }
    
    async getAreaTransport(bounds) {
        const results = await Promise.all([
            this.trackers.flights.getAreaFlights(bounds),
            this.trackers.transit.getAreaTransit(bounds),
            this.trackers.traffic.getAreaTraffic(bounds),
            this.trackers.bikes.getAreaStations(bounds),
            this.trackers.ships.getAreaShips(bounds)
        ]);
        
        return {
            flights: results[0],
            transit: results[1],
            traffic: results[2],
            bikes: results[3],
            ships: results[4],
            timestamp: new Date().toISOString()
        };
    }
    
    startRealTimeUpdates(bounds, callback) {
        // Update at different intervals based on data type
        setInterval(() => {
            this.trackers.flights.getAreaFlights(bounds)
                .then(data => callback('flights', data));
        }, 10000); // 10 seconds
        
        setInterval(() => {
            this.trackers.transit.getAreaTransit(bounds)
                .then(data => callback('transit', data));
        }, 30000); // 30 seconds
        
        setInterval(() => {
            this.trackers.bikes.getAreaStations(bounds)
                .then(data => callback('bikes', data));
        }, 60000); // 1 minute
    }
}
```

---

## Best Practices

### 1. Efficient Polling
```python
import asyncio
from datetime import datetime, timedelta

class EfficientPoller:
    def __init__(self, sources):
        self.sources = sources
        self.last_update = {}
        
    async def poll_source(self, source):
        # Different update intervals for different sources
        intervals = {
            'flights': 10,
            'transit': 30,
            'bikes': 60,
            'traffic': 300
        }
        
        interval = intervals.get(source.type, 60)
        
        while True:
            try:
                data = await source.fetch()
                self.last_update[source.name] = {
                    'time': datetime.now(),
                    'data': data
                }
                await asyncio.sleep(interval)
            except Exception as e:
                print(f"Error polling {source.name}: {e}")
                await asyncio.sleep(interval * 2)  # Back off on error
    
    async def start(self):
        tasks = [self.poll_source(source) for source in self.sources]
        await asyncio.gather(*tasks)
```

### 2. Data Deduplication
```javascript
class TransportDataDeduplicator {
    constructor() {
        this.seenHashes = new Map();
        this.ttl = 3600000; // 1 hour
    }
    
    hash(data) {
        // Create unique hash for transport object
        return `${data.type}-${data.id}-${data.lat}-${data.lon}-${data.timestamp}`;
    }
    
    isDuplicate(data) {
        const hash = this.hash(data);
        const seen = this.seenHashes.get(hash);
        
        if (seen && Date.now() - seen < this.ttl) {
            return true;
        }
        
        this.seenHashes.set(hash, Date.now());
        this.cleanup();
        return false;
    }
    
    cleanup() {
        const now = Date.now();
        for (const [hash, time] of this.seenHashes) {
            if (now - time > this.ttl) {
                this.seenHashes.delete(hash);
            }
        }
    }
}
```

---

## Comparison Matrix

| Source | Type | Coverage | Update Rate | Free Tier | Best For |
|--------|------|----------|-------------|-----------|----------|
| OpenSky | Flights | Global | 5-10s | 4000/day | Flight tracking |
| GTFS-RT | Transit | 1000+ cities | 10-30s | Unlimited | Public transit |
| Marine AIS | Ships | Regional | Real-time | Varies | Maritime |
| HERE/TomTom | Traffic | Global | Minutes | Limited | Road traffic |
| GBFS | Bikes | 500+ cities | 30s | Unlimited | Bike shares |
| 511.org | Traffic | California | Real-time | Unlimited | CA traffic |