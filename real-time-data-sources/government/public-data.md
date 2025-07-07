# Government & Public Real-Time Data Sources

## US Government Data ⭐⭐⭐⭐⭐

### Data.gov Real-Time APIs
- **Portal**: https://catalog.data.gov/dataset?res_format=api
- **Coverage**: All US federal agencies
- **Authentication**: Usually none or free API key
- **Update Frequency**: Varies by dataset

#### Key Real-Time Feeds

##### USDA Agricultural Data
```python
import requests
from datetime import datetime

class USDADataClient:
    def __init__(self, api_key):
        self.api_key = api_key
        self.base_url = 'https://api.nal.usda.gov/fdc/v1'
    
    def search_foods(self, query, limit=50):
        """Search food database"""
        url = f'{self.base_url}/foods/search'
        params = {
            'api_key': self.api_key,
            'query': query,
            'limit': limit
        }
        response = requests.get(url, params=params)
        return response.json()
    
    def get_market_news(self):
        """Get agricultural market news"""
        # USDA Market News API
        url = 'https://marsapi.ams.usda.gov/services/v1.2/reports'
        response = requests.get(url)
        return response.json()

# Commodity prices
class USDAMarketData:
    def __init__(self):
        self.base_url = 'https://marsapi.ams.usda.gov/services/v1.2'
    
    def get_reports(self, commodity='cattle'):
        """Get market reports"""
        url = f'{self.base_url}/reports'
        params = {'commodity': commodity}
        response = requests.get(url, params=params)
        return response.json()
    
    def get_report_data(self, report_id):
        """Get specific report data"""
        url = f'{self.base_url}/reports/{report_id}'
        response = requests.get(url)
        return response.json()
```

##### US Treasury Rates
```javascript
// Real-time treasury data
class TreasuryDataClient {
    constructor() {
        this.baseUrl = 'https://api.fiscaldata.treasury.gov/services/api/fiscal_service';
    }
    
    async getDailyTreasuryRates() {
        const url = `${this.baseUrl}/v2/accounting/od/avg_interest_rates`;
        const params = new URLSearchParams({
            filter: `record_date:gte:${this.getLastBusinessDay()}`,
            sort: '-record_date'
        });
        
        const response = await fetch(`${url}?${params}`);
        return response.json();
    }
    
    async getTreasuryBalance() {
        const url = `${this.baseUrl}/v1/accounting/dts/operating_cash_balance`;
        const params = new URLSearchParams({
            filter: `record_date:eq:${this.getToday()}`,
            format: 'json'
        });
        
        const response = await fetch(`${url}?${params}`);
        return response.json();
    }
    
    getToday() {
        return new Date().toISOString().split('T')[0];
    }
    
    getLastBusinessDay() {
        const date = new Date();
        date.setDate(date.getDate() - 7); // Last week
        return date.toISOString().split('T')[0];
    }
}
```

##### FDA Recalls & Safety Alerts
```python
class FDARecallMonitor:
    def __init__(self):
        self.base_url = 'https://api.fda.gov'
        self.known_recalls = set()
    
    def get_recent_recalls(self, days=7):
        """Get recent FDA recalls"""
        url = f'{self.base_url}/food/enforcement.json'
        params = {
            'search': f'report_date:[{self.get_date(days)} TO NOW]',
            'limit': 100
        }
        
        response = requests.get(url, params=params)
        data = response.json()
        return data.get('results', [])
    
    def monitor_new_recalls(self):
        """Check for new recalls"""
        recalls = self.get_recent_recalls(1)
        
        for recall in recalls:
            recall_id = recall['recall_number']
            if recall_id not in self.known_recalls:
                self.known_recalls.add(recall_id)
                self.alert_new_recall(recall)
    
    def alert_new_recall(self, recall):
        print(f"🚨 NEW RECALL: {recall['product_description']}")
        print(f"   Reason: {recall['reason_for_recall']}")
        print(f"   Distribution: {recall['distribution_pattern']}")
        print(f"   Date: {recall['report_date']}")
    
    def get_date(self, days_ago):
        from datetime import datetime, timedelta
        date = datetime.now() - timedelta(days=days_ago)
        return date.strftime('%Y-%m-%d')
```

---

## European Union Open Data ⭐⭐⭐⭐

### European Data Portal
- **Portal**: https://data.europa.eu/
- **API**: https://data.europa.eu/api/
- **Coverage**: All EU member states
- **Authentication**: None for most datasets

#### Real-Time EU Datasets

##### European Energy Exchange
```javascript
// EU electricity prices
class EuropeanEnergyData {
    constructor() {
        this.entsoeToken = 'YOUR_ENTSOE_TOKEN'; // Register at transparency.entsoe.eu
    }
    
    async getElectricityPrices(country, periodStart, periodEnd) {
        const url = 'https://transparency.entsoe.eu/api';
        const params = new URLSearchParams({
            securityToken: this.entsoeToken,
            documentType: 'A44', // Price document
            in_Domain: this.getCountryCode(country),
            out_Domain: this.getCountryCode(country),
            periodStart: periodStart, // YYYYMMDDHHMM
            periodEnd: periodEnd
        });
        
        const response = await fetch(`${url}?${params}`);
        const xml = await response.text();
        return this.parseXMLResponse(xml);
    }
    
    getCountryCode(country) {
        const codes = {
            'DE': '10Y1001A1001A83F', // Germany
            'FR': '10YFR-RTE------C', // France
            'ES': '10YES-REE------0', // Spain
            'IT': '10YIT-GRTN-----B', // Italy
            // Add more country codes
        };
        return codes[country];
    }
}
```

##### EU Air Quality
```python
# European Environment Agency air quality
class EEAAirQuality:
    def __init__(self):
        self.base_url = 'https://discomap.eea.europa.eu/map/fme/AirQualityExport.htm'
    
    def get_latest_data(self, country_code, city=None, pollutant='PM2.5'):
        """Get latest air quality measurements"""
        params = {
            'CountryCode': country_code,
            'Pollutant': pollutant,
            'Year_from': datetime.now().year,
            'Year_to': datetime.now().year,
            'Station': '',
            'Samplingpoint': '',
            'Source': 'E1a',  # Real-time data
            'Output': 'XML',
            'UpdateDate': ''
        }
        
        if city:
            params['City'] = city
        
        response = requests.get(self.base_url, params=params)
        return self.parse_xml_response(response.content)
```

---

## NASA Space & Earth Data ⭐⭐⭐⭐⭐

### NASA Open APIs
- **Portal**: https://api.nasa.gov/
- **Authentication**: Free API key
- **Rate Limit**: 1000 requests/hour

#### Real-Time Space Data

##### ISS Location
```python
class NASADataClient:
    def __init__(self, api_key):
        self.api_key = api_key
        self.base_url = 'https://api.nasa.gov'
    
    def get_iss_location(self):
        """Get current ISS position"""
        # This endpoint doesn't require API key
        url = 'http://api.open-notify.org/iss-now.json'
        response = requests.get(url)
        data = response.json()
        
        return {
            'timestamp': data['timestamp'],
            'latitude': float(data['iss_position']['latitude']),
            'longitude': float(data['iss_position']['longitude'])
        }
    
    def get_people_in_space(self):
        """Get astronauts currently in space"""
        url = 'http://api.open-notify.org/astros.json'
        response = requests.get(url)
        return response.json()
    
    def get_solar_flare_data(self, start_date=None):
        """Get solar flare activity"""
        if not start_date:
            start_date = (datetime.now() - timedelta(days=30)).strftime('%Y-%m-%d')
        
        url = f'{self.base_url}/DONKI/FLR'
        params = {
            'startDate': start_date,
            'api_key': self.api_key
        }
        
        response = requests.get(url, params=params)
        return response.json()
    
    def get_neo_feed(self):
        """Get Near Earth Objects"""
        url = f'{self.base_url}/neo/rest/v1/feed'
        params = {
            'api_key': self.api_key,
            'detailed': 'true'
        }
        
        response = requests.get(url, params=params)
        return response.json()
```

##### Earth Observation Data
```javascript
// NASA Earthdata streaming
class NASAEarthdata {
    constructor(username, password) {
        this.auth = Buffer.from(`${username}:${password}`).toString('base64');
        this.baseUrl = 'https://cmr.earthdata.nasa.gov';
    }
    
    async searchGranules(params) {
        const url = `${this.baseUrl}/search/granules.json`;
        const queryParams = new URLSearchParams({
            short_name: params.dataset,
            temporal: params.temporalRange,
            bounding_box: params.boundingBox,
            page_size: 100,
            sort_key: '-start_date'
        });
        
        const response = await fetch(`${url}?${queryParams}`, {
            headers: {
                'Authorization': `Basic ${this.auth}`
            }
        });
        
        return response.json();
    }
    
    subscribeToDataset(datasetId, callback) {
        // Use CMR's subscription service for real-time updates
        const subscriptionUrl = `${this.baseUrl}/search/subscriptions`;
        
        const subscription = {
            name: `Subscription_${Date.now()}`,
            collection_concept_id: datasetId,
            query: 'updated_since=true',
            notify_url: callback
        };
        
        return fetch(subscriptionUrl, {
            method: 'POST',
            headers: {
                'Authorization': `Basic ${this.auth}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(subscription)
        });
    }
}
```

---

## World Bank Data ⭐⭐⭐⭐

### World Bank API
- **Base URL**: https://api.worldbank.org/v2/
- **Format**: JSON, XML
- **Authentication**: None required
- **Update**: Monthly/Quarterly

```python
class WorldBankData:
    def __init__(self):
        self.base_url = 'https://api.worldbank.org/v2'
    
    def get_indicator_data(self, indicator, countries='all', date_range='2020:2024'):
        """Get economic indicator data"""
        url = f'{self.base_url}/country/{countries}/indicator/{indicator}'
        params = {
            'format': 'json',
            'date': date_range,
            'per_page': 1000
        }
        
        response = requests.get(url, params=params)
        return response.json()
    
    def get_gdp_data(self, countries='all'):
        """Get GDP data"""
        return self.get_indicator_data('NY.GDP.MKTP.CD', countries)
    
    def get_inflation_data(self, countries='all'):
        """Get inflation data"""
        return self.get_indicator_data('FP.CPI.TOTL.ZG', countries)
    
    def get_unemployment_data(self, countries='all'):
        """Get unemployment data"""
        return self.get_indicator_data('SL.UEM.TOTL.ZS', countries)
    
    def stream_updates(self, indicators, interval=3600):
        """Poll for updates at regular intervals"""
        import time
        
        while True:
            for indicator in indicators:
                data = self.get_indicator_data(indicator)
                yield {
                    'indicator': indicator,
                    'data': data,
                    'timestamp': datetime.now().isoformat()
                }
            
            time.sleep(interval)
```

---

## UN Data APIs ⭐⭐⭐

### UN Comtrade (Trade Data)
```javascript
class UNComtradeAPI {
    constructor() {
        this.baseUrl = 'https://comtrade.un.org/api/get';
    }
    
    async getTradeData(params) {
        const defaultParams = {
            max: 50000,
            type: 'C',
            freq: 'M', // Monthly
            px: 'HS', // Harmonized System
            ps: 'recent', // Recent periods
            r: 'all', // Reporters
            p: 'all', // Partners
            rg: '2', // Imports
            cc: 'TOTAL', // All commodities
            fmt: 'json'
        };
        
        const queryParams = { ...defaultParams, ...params };
        const url = `${this.baseUrl}?${new URLSearchParams(queryParams)}`;
        
        const response = await fetch(url);
        return response.json();
    }
    
    async getRecentImports(reporterCode) {
        return this.getTradeData({
            r: reporterCode,
            ps: 'recent',
            rg: '1' // Imports
        });
    }
    
    async getRecentExports(reporterCode) {
        return this.getTradeData({
            r: reporterCode,
            ps: 'recent',
            rg: '2' // Exports
        });
    }
}
```

---

## Central Banks Data ⭐⭐⭐⭐

### Federal Reserve Economic Data (FRED)
```python
class FREDClient:
    def __init__(self, api_key):
        self.api_key = api_key
        self.base_url = 'https://api.stlouisfed.org/fred'
    
    def get_series(self, series_id, realtime_start=None):
        """Get economic time series data"""
        url = f'{self.base_url}/series/observations'
        params = {
            'series_id': series_id,
            'api_key': self.api_key,
            'file_type': 'json',
            'sort_order': 'desc',
            'limit': 100
        }
        
        if realtime_start:
            params['realtime_start'] = realtime_start
        
        response = requests.get(url, params=params)
        return response.json()
    
    def get_latest_value(self, series_id):
        """Get most recent value for a series"""
        data = self.get_series(series_id)
        if data['observations']:
            return {
                'series': series_id,
                'date': data['observations'][0]['date'],
                'value': data['observations'][0]['value']
            }
        return None
    
    # Common series
    def get_gdp(self):
        return self.get_latest_value('GDP')
    
    def get_unemployment_rate(self):
        return self.get_latest_value('UNRATE')
    
    def get_inflation_rate(self):
        return self.get_latest_value('CPIAUCSL')
    
    def get_fed_funds_rate(self):
        return self.get_latest_value('FEDFUNDS')
```

### European Central Bank
```javascript
// ECB Statistical Data Warehouse
class ECBDataClient {
    constructor() {
        this.baseUrl = 'https://sdw-wsrest.ecb.europa.eu/service';
    }
    
    async getExchangeRates(currency = 'USD') {
        const key = `EXR.D.${currency}.EUR.SP00.A`;
        const url = `${this.baseUrl}/data/${key}`;
        
        const response = await fetch(url, {
            headers: {
                'Accept': 'application/json'
            }
        });
        
        return response.json();
    }
    
    async getInterestRates() {
        const key = 'FM.D.U2.EUR.4F.KR.MRR_MBR.LEV';
        const url = `${this.baseUrl}/data/${key}`;
        
        const response = await fetch(url, {
            headers: {
                'Accept': 'application/json'
            }
        });
        
        return response.json();
    }
    
    async getInflationData() {
        const key = 'ICP.M.U2.N.000000.4.ANR';
        const url = `${this.baseUrl}/data/${key}`;
        
        const response = await fetch(url, {
            headers: {
                'Accept': 'application/json'
            }
        });
        
        return response.json();
    }
}
```

---

## Public Health Data ⭐⭐⭐⭐

### CDC Data APIs
```python
class CDCDataClient:
    def __init__(self):
        self.base_url = 'https://data.cdc.gov/resource'
    
    def get_covid_data(self, state=None):
        """Get COVID-19 case data"""
        endpoint = '9mfq-cb36.json'  # COVID-19 case surveillance
        url = f'{self.base_url}/{endpoint}'
        
        params = {
            '$limit': 1000,
            '$order': 'created_at DESC'
        }
        
        if state:
            params['$where'] = f"res_state='{state}'"
        
        response = requests.get(url, params=params)
        return response.json()
    
    def get_flu_activity(self):
        """Get weekly flu activity"""
        endpoint = 'y9kf-t6qx.json'  # FluView
        url = f'{self.base_url}/{endpoint}'
        
        params = {
            '$limit': 100,
            '$order': 'week DESC'
        }
        
        response = requests.get(url, params=params)
        return response.json()
    
    def get_vaccination_coverage(self):
        """Get vaccination coverage data"""
        endpoint = 'unsk-b7fc.json'
        url = f'{self.base_url}/{endpoint}'
        
        response = requests.get(url)
        return response.json()
```

### WHO Global Health Observatory
```javascript
class WHODataClient {
    constructor() {
        this.baseUrl = 'https://ghoapi.azureedge.net/api';
    }
    
    async getIndicator(indicatorCode) {
        const url = `${this.baseUrl}/${indicatorCode}`;
        const response = await fetch(url);
        return response.json();
    }
    
    async getCountryData(countryCode, indicatorCode) {
        const url = `${this.baseUrl}/${indicatorCode}?$filter=SpatialDim eq '${countryCode}'`;
        const response = await fetch(url);
        return response.json();
    }
    
    async getCovidData() {
        // COVID-19 cases
        return this.getIndicator('COVID_19_CASES');
    }
    
    async getLifeExpectancy(countryCode) {
        return this.getCountryData(countryCode, 'WHOSIS_000001');
    }
}
```

---

## Integration Framework

```python
import asyncio
from datetime import datetime
import aiohttp

class GovernmentDataAggregator:
    def __init__(self, credentials):
        self.credentials = credentials
        self.sources = {
            'treasury': TreasuryDataClient(),
            'fred': FREDClient(credentials['fred_api_key']),
            'nasa': NASADataClient(credentials['nasa_api_key']),
            'usda': USDADataClient(credentials['usda_api_key']),
            'cdc': CDCDataClient(),
            'world_bank': WorldBankData()
        }
        
        self.update_intervals = {
            'treasury': 3600,      # 1 hour
            'fred': 3600,          # 1 hour
            'nasa': 300,           # 5 minutes
            'usda': 21600,         # 6 hours
            'cdc': 86400,          # 24 hours
            'world_bank': 86400    # 24 hours
        }
    
    async def fetch_data(self, source_name):
        """Fetch data from a specific source"""
        source = self.sources[source_name]
        
        if source_name == 'treasury':
            return await self.fetch_treasury_data(source)
        elif source_name == 'fred':
            return await self.fetch_fred_data(source)
        # ... implement other sources
    
    async def fetch_treasury_data(self, client):
        """Fetch treasury specific data"""
        return {
            'rates': await client.getDailyTreasuryRates(),
            'balance': await client.getTreasuryBalance(),
            'timestamp': datetime.now().isoformat()
        }
    
    async def start_monitoring(self):
        """Start monitoring all sources"""
        tasks = []
        
        for source_name, interval in self.update_intervals.items():
            task = asyncio.create_task(
                self.monitor_source(source_name, interval)
            )
            tasks.append(task)
        
        await asyncio.gather(*tasks)
    
    async def monitor_source(self, source_name, interval):
        """Monitor a single source"""
        while True:
            try:
                data = await self.fetch_data(source_name)
                await self.process_data(source_name, data)
            except Exception as e:
                print(f"Error fetching {source_name}: {e}")
            
            await asyncio.sleep(interval)
    
    async def process_data(self, source_name, data):
        """Process and store fetched data"""
        # Implement data processing logic
        print(f"Updated {source_name} at {datetime.now()}")
```

---

## Best Practices

### 1. Rate Limit Compliance
- Respect API rate limits
- Implement exponential backoff
- Cache responses appropriately
- Use bulk endpoints when available

### 2. Data Quality
- Validate data timestamps
- Check for missing values
- Handle API changes gracefully
- Monitor data quality metrics

### 3. Legal Compliance
- Read terms of service
- Attribute data sources
- Respect data licenses
- Handle PII appropriately

---

## Comparison Matrix

| Source | Update Frequency | Authentication | Rate Limits | Best For |
|--------|------------------|----------------|-------------|----------|
| NOAA/NWS | 5-60 min | None | None | US weather |
| NASA | Real-time to daily | API key | 1000/hour | Space data |
| FRED | Daily | API key | 120/min | Economic data |
| Data.gov | Varies | Varies | Varies | US gov data |
| EU Portal | Varies | None | Reasonable | EU data |
| World Bank | Monthly | None | None | Global indicators |