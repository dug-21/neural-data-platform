# Alternative Data Sources for Long-Term Investing

> Research Document | Neural Data Platform - Financial Intelligence Module
>
> **Focus**: Unconventional data providing non-obvious signals for market direction
> **Investment Horizon**: Long-term (weeks/months)
> **Deployment Target**: Raspberry Pi edge computing

---

## Executive Summary

Alternative data has become a $14+ billion industry, with 67% of institutional investors now using non-traditional data sources. However, most high-quality alternative data remains expensive and inaccessible to retail investors. This research identifies **free and low-cost alternatives** that can provide leading indicator signals for sector rotation and risk-on/risk-off timing decisions.

**Key Findings:**
1. **Google Trends** and **FRED economic data** offer the best value for edge deployment
2. **Reddit sentiment** shows weak direct correlation but strong volume signals
3. **Baltic Dry Index** provides genuine leading indicator capability for free
4. **SEC EDGAR** offers untapped sentiment signals via filing analysis
5. **Satellite/foot traffic** data is largely inaccessible without institutional subscriptions

---

## Table of Contents

1. [Alternative Data Categories](#1-alternative-data-categories)
2. [Free and Low-Cost Data Sources](#2-free-and-low-cost-data-sources)
3. [Non-Obvious Correlations](#3-non-obvious-correlations)
4. [Lead Time Assessment](#4-lead-time-assessment)
5. [Edge Deployment Feasibility](#5-edge-deployment-feasibility)
6. [Recommendations for NDP Integration](#6-recommendations-for-ndp-integration)

---

## 1. Alternative Data Categories

### 1.1 Satellite Imagery

**What It Measures:**
- Parking lot occupancy at retail stores
- Shipping container volumes at ports
- Agricultural crop health and yields
- Oil storage tank levels

**Correlation Evidence:**
Research using 4.9 million daily observations from 44 major US retailers (2011-2017) found:
- Parking lot utilization predicts same-store sales growth with **correlation coefficient of 0.78**
- Trading strategy yielded **4.95% returns** in 3-day windows around earnings
- One standard deviation increase in parking lot utilization correlates with **2.2% sales increase**

**Providers:**
| Provider | Type | Cost | Notes |
|----------|------|------|-------|
| [Orbital Insight](https://orbitalinsight.com) | Commercial | $50K+/year | 70/74 clients are hedge funds |
| [RS Metrics](https://rsmetrics.com) | Commercial | Institutional | Pioneer in parking lot analysis |
| [Planet Labs](https://planet.com) | Commercial | $5K+/month | Daily satellite imagery |

**Accessibility for Retail Investors:** **LOW** - No free options available. Google Earth historical imagery is too infrequent for trading signals.

### 1.2 Credit Card Transaction Data

**What It Measures:**
- Real-time consumer spending by merchant
- Category-level spending trends
- Geographic spending patterns

**Market Position:**
Credit/debit card transactions dominate the alternative data market with **17.2% revenue share** in 2024.

**Correlation Evidence:**
- Predicts earnings surprises **2-3 weeks earlier** than traditional forecasts
- Provides near real-time view of company sales trends

**Providers:**
| Provider | Type | Cost | Notes |
|----------|------|------|-------|
| Second Measure | Commercial | Institutional | Anonymized transaction data |
| Yodlee | Commercial | Institutional | 20M+ cardholders tracked |
| M Science | Commercial | $100K+/year | Consumer transaction panels |

**Accessibility for Retail Investors:** **NONE** - Entirely institutional. Privacy regulations prevent retail access.

### 1.3 Web Traffic and App Usage

**What It Measures:**
- Website visitor counts and engagement
- App downloads and daily active users
- Search trends and interest patterns

**Correlation Evidence:**
- Strong correlation between web traffic changes and company revenue
- App download trends predict earnings 1-2 quarters ahead for tech companies

**Providers:**
| Provider | Type | Cost | Notes |
|----------|------|------|-------|
| SimilarWeb | Freemium | $0-$10K/month | Limited free tier |
| App Annie (data.ai) | Freemium | $0-$5K/month | App store analytics |
| **Google Trends** | **Free** | $0 | Retail-accessible |

**Accessibility for Retail Investors:** **MEDIUM** - Google Trends is free and powerful; SimilarWeb has limited free tier.

### 1.4 Job Postings and LinkedIn Activity

**What It Measures:**
- Company hiring velocity
- Skills demand shifts
- Industry growth/contraction signals

**Lead/Lag Characteristics:**
- Traditional job reports are **lagging indicators** (2-3 quarters after economic turns)
- Real-time job posting data from LinkedIn/Indeed can be **leading** by 1-2 months

**Providers:**
| Provider | Type | Cost | Notes |
|----------|------|------|-------|
| [LinkedIn Economic Graph](https://economicgraph.linkedin.com/workforce-data) | Free reports | $0 | Aggregated insights |
| Indeed Hiring Lab | Free reports | $0 | Monthly job posting trends |
| Thinknum | Commercial | $30K+/year | Real-time job posting API |

**Accessibility for Retail Investors:** **MEDIUM** - Free reports available; real-time data requires subscription.

### 1.5 Supply Chain Data

**What It Measures:**
- Shipping costs and volumes
- Container port activity
- Raw material demand

**Key Index: Baltic Dry Index (BDI)**

The BDI measures the cost of transporting raw materials globally. It is considered a **genuine leading indicator** because:
- "Totally devoid of speculative content" - people don't book freighters without cargo
- 2-3 year lead time to add new shipping capacity makes it highly inelastic
- Predicts GDP growth and industrial production

**Accessibility for Retail Investors:** **HIGH** - BDI is freely available on [TradingEconomics](https://tradingeconomics.com/commodity/baltic) and [TradingView](https://www.tradingview.com/symbols/INDEX-BDI/).

### 1.6 Patent Filings

**What It Measures:**
- Innovation pipeline
- R&D output
- Competitive positioning

**Correlation Evidence:**
- Companies in top quintile by patent grants achieved **4.5% excess returns**
- **4.1% return spread** between top and bottom quintiles
- Top quintiles outperformed in **99-100%** of 5-year periods (1990-2017)

**Data Source:**
[USPTO Research Datasets](https://www.uspto.gov/ip-policy/economic-research/research-datasets) - **FREE** public access to all patent filing data.

**Accessibility for Retail Investors:** **HIGH** - All USPTO data is free and publicly accessible.

### 1.7 Geolocation/Foot Traffic

**What It Measures:**
- Store visitor counts
- Dwell time
- Cross-shopping behavior

**Providers:**
| Provider | Type | Cost | Notes |
|----------|------|------|-------|
| Placer.ai | Freemium | $200-$2K/month | Limited free insights |
| SafeGraph | Academic free | $0 for researchers | COVID consortium access |
| Google Popular Times | Free | $0 | Very limited data |

**Accessibility for Retail Investors:** **LOW** - Meaningful data requires paid subscriptions.

### 1.8 Social Media Sentiment

**What It Measures:**
- Public opinion on companies/products
- Retail investor attention
- Meme stock momentum

**Correlation Evidence (Mixed):**
- 2018 study: Twitter sentiment predicted stock movements **up to 6 days ahead** with 87% accuracy
- 2024 WSB research: Sentiment shows **weak correlation** with stock prices
- **Volume of comments and Google search trends show stronger signals** than sentiment scores
- WSB-attention positions realized **-8.5% holding period returns** (caution signal)

**Free Providers:**
| Provider | Type | Cost | Notes |
|----------|------|------|-------|
| [ApeWisdom](https://apewisdom.io/api/) | Free API | $0 | Reddit/4chan stock mentions |
| StockTwits | Freemium | $0 | Social sentiment |
| Twitter/X API | Freemium | $0-$100/month | Academic/basic tiers |

**Accessibility for Retail Investors:** **HIGH** - Multiple free options, but signal quality is questionable.

---

## 2. Free and Low-Cost Data Sources

### 2.1 Government Data (Tier 1 - Highest Value)

#### FRED (Federal Reserve Economic Data)
**URL:** [fred.stlouisfed.org](https://fred.stlouisfed.org)

| Feature | Details |
|---------|---------|
| Data Series | 921 leading indicators, 57 leading indices |
| History | 1980 to present with 5-year projections |
| API | Free, no authentication required |
| Update Frequency | Daily to monthly depending on series |

**Key Leading Indicators on FRED:**
- Composite Leading Indicator (CLI) - Predicts turning points by ~7 months
- GDP-Based Recession Indicator - Above 67% = recession, below 33% = recovery
- Initial Jobless Claims - Weekly leading indicator
- Building Permits - 2-3 quarter lead on housing/construction
- Yield Curve (10Y-2Y spread) - 12-18 month recession predictor

**FRED-MD/FRED-QD Research Databases:**
Large macroeconomic databases designed for "big data" empirical analysis, updated in real-time and publicly accessible.

#### SEC EDGAR
**URL:** [sec.gov/edgar](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)

| Feature | Details |
|---------|---------|
| API | Free, no authentication required |
| Data | 10-K, 10-Q, 8-K, insider trading (Form 4) |
| History | 1993 to present |
| Format | JSON via data.sec.gov |

**Python Library:** [EdgarTools](https://github.com/dgunning/edgartools) - Download and analyze SEC filings, extract XBRL financial statements.

#### Bureau of Labor Statistics (BLS)
**URL:** [bls.gov/data](https://www.bls.gov/data)

- Employment statistics
- Consumer Price Index (CPI)
- Producer Price Index (PPI)
- Productivity data

### 2.2 International Economic Data (Tier 1)

| Source | URL | Key Data |
|--------|-----|----------|
| **IMF Data** | [data.imf.org](https://data.imf.org) | World Economic Outlook, global projections |
| **World Bank** | [data.worldbank.org](https://data.worldbank.org) | Development indicators, 200+ countries |
| **OECD** | [data-explorer.oecd.org](https://data-explorer.oecd.org) | 38 OECD countries' statistics |
| **DBnomics** | [db.nomics.world](https://db.nomics.world) | Aggregates all above sources |

### 2.3 Market Data APIs (Tier 2)

| Provider | Free Tier | Rate Limits | Best For |
|----------|-----------|-------------|----------|
| [Alpha Vantage](https://www.alphavantage.co) | 25 calls/day | 5/minute | Historical prices, fundamentals |
| [Finnhub](https://finnhub.io) | 60 calls/minute | Limited symbols | Real-time quotes, news sentiment |
| [Financial Modeling Prep](https://site.financialmodelingprep.com) | 250 calls/day | 5/second | Financial statements |
| [Yahoo Finance (yfinance)](https://pypi.org/project/yfinance/) | Unlimited | Unofficial | Historical data, no API key |

### 2.4 Sentiment and Search Data (Tier 2)

#### Google Trends
**URL:** [trends.google.com](https://trends.google.com)

**Research Findings:**
- Strong correlation between search volume for financial keywords and stock trading volume
- Search peaks for "debt" correlate with S&P 500 declines
- Search peaks for "stock market" precede major crashes
- Peaks for "buy stocks [ticker]" correlate with positive returns

**Limitations:**
- During uncertainty, searches increase without buying intent
- Sentiment of search terms matters (bullish vs. bearish keywords)

**Python Library:** [pytrends](https://pypi.org/project/pytrends/) - Unofficial API for Google Trends data.

#### Reddit Sentiment
**URL:** [apewisdom.io/api](https://apewisdom.io/api/)

| Metric | Signal Quality | Notes |
|--------|---------------|-------|
| Sentiment score | **Weak** | Poor correlation with returns |
| Comment volume | **Moderate** | Better predictor than sentiment |
| Ticker mentions | **Moderate** | Tracks retail attention |
| Emoji usage | **Interesting** | Price predicts emoji usage (reverse causation) |

**Caution:** Research shows WSB-attention positions average **-8.5% returns**. This may be a **contrarian indicator**.

### 2.5 Shipping and Trade Data (Tier 1)

#### Baltic Dry Index
**Free Sources:**
- [TradingEconomics](https://tradingeconomics.com/commodity/baltic) - Daily updates, free charts
- [TradingView](https://www.tradingview.com/symbols/INDEX-BDI/) - Real-time, free account

**Why It Matters:**
- No speculative component (real cargo bookings only)
- Highly inelastic supply (2-3 year lag to add capacity)
- Leading indicator for global trade and GDP
- Key findings: S&P 500 is most correlated variable, followed by iron ore and coal indices

### 2.6 Innovation and Technology Data (Tier 2)

#### USPTO Patent Data
**URL:** [uspto.gov/ip-policy/economic-research/research-datasets](https://www.uspto.gov/ip-policy/economic-research/research-datasets)

- Full patent filing history
- Searchable by company, technology, inventor
- AI-related patents grew **40%** from 2019-2020

---

## 3. Non-Obvious Correlations

### 3.1 Documented Alpha-Generating Correlations

| Data Source | Correlation | Lead Time | Documented Returns |
|-------------|-------------|-----------|-------------------|
| Satellite parking lot data | Same-store sales | 2-3 weeks pre-earnings | 4.95% in 3 days |
| Credit card transactions | Earnings surprises | 2-3 weeks pre-earnings | Not disclosed |
| Patent grant counts | Excess returns | Quarterly | 4.1% spread |
| Baltic Dry Index | GDP growth | 3-6 months | Leading indicator |
| Google "debt" searches | S&P 500 direction | 1-4 weeks | Negative correlation |
| Initial jobless claims | Recession timing | 6-12 months | Leading indicator |
| Yield curve inversion | Recession | 12-18 months | Historical accuracy >80% |

### 3.2 Academic Research Summary

**"101 Formulaic Alphas" by Zura Kakushadze:**
Collection of 101 trading formulas based on price, volume, and market variables developed at a quantitative hedge fund.

**Key Findings from Academic Literature:**
- Ensemble ML methods (XGBoost, Random Forest) achieve **up to 86% directional accuracy**
- 1% improvement in directional accuracy generates significant alpha at scale
- Hybrid data (combining traditional + alternative) outperforms single-source approaches
- Operational metrics improve earnings prediction accuracy by **18%** (McKinsey 2023)

### 3.3 Contrarian Signals

**Reddit WSB Attention as Contrarian Indicator:**
- High WSB attention correlates with **negative** holding period returns
- Positions created at peak WSB attention: **-8.5% average returns**
- This suggests WSB attention may be useful as a **fade signal**

---

## 4. Lead Time Assessment

### 4.1 Leading Indicators (Ahead of Price)

| Indicator | Typical Lead Time | Data Frequency | Free Access |
|-----------|------------------|----------------|-------------|
| Yield Curve (10Y-2Y) | 12-18 months | Daily | Yes (FRED) |
| Baltic Dry Index | 3-6 months | Daily | Yes |
| OECD Composite Leading Index | ~7 months | Monthly | Yes |
| Building Permits | 2-3 quarters | Monthly | Yes (FRED) |
| Initial Jobless Claims | 6-12 months | Weekly | Yes (FRED) |
| Google Trends (fear keywords) | 1-4 weeks | Daily/Weekly | Yes |
| ISM Manufacturing PMI | 2-3 months | Monthly | Yes (FRED) |
| Consumer Confidence | 1-2 quarters | Monthly | Yes (FRED) |

### 4.2 Coincident Indicators

| Indicator | Timing | Data Frequency | Free Access |
|-----------|--------|----------------|-------------|
| Nonfarm Payrolls | Real-time economy | Monthly | Yes (FRED) |
| Industrial Production | Real-time | Monthly | Yes (FRED) |
| Retail Sales | Real-time | Monthly | Yes (FRED) |
| GDP (first estimate) | Quarterly lag | Quarterly | Yes (FRED) |

### 4.3 Lagging Indicators

| Indicator | Typical Lag | Notes |
|-----------|-------------|-------|
| Unemployment Rate | 2-3 quarters | Rises after recession starts |
| CPI/Inflation | 3-6 months | Policy response lag |
| Corporate Earnings | 1 quarter | Backward-looking by definition |
| Traditional Job Reports | 2-3 quarters | Government data collection lag |

### 4.4 Lead Time Summary for Long-Term Investing

For **sector rotation** and **risk-on/risk-off** decisions with weeks/months horizon:

**Most Useful (Free, Leading, Weeks-Months Lead):**
1. Yield Curve - 12-18 month recession warning
2. Baltic Dry Index - 3-6 month trade/GDP signal
3. Initial Jobless Claims - 6-12 month economic health
4. OECD CLI - 7 month business cycle signal
5. Google Trends (economic fear terms) - 1-4 week sentiment shifts

---

## 5. Edge Deployment Feasibility

### 5.1 Raspberry Pi Constraints

| Resource | Raspberry Pi 4 (4GB) | Raspberry Pi 5 (8GB) |
|----------|---------------------|---------------------|
| CPU | Quad-core 1.5GHz | Quad-core 2.4GHz |
| RAM | 4GB | 8GB |
| Storage | microSD (slow) / USB SSD | microSD / USB SSD |
| Network | 1Gbps Ethernet, WiFi | 1Gbps Ethernet, WiFi |
| Power | 5W typical | 7W typical |

### 5.2 Data Source Feasibility Matrix

| Data Source | Daily Volume | API Rate Limits | Pi Feasible | Notes |
|-------------|--------------|-----------------|-------------|-------|
| FRED API | <1MB/day | 120 req/min | **Excellent** | Batch daily |
| SEC EDGAR | 10-100MB/day | No limit | **Good** | Selective parsing |
| Alpha Vantage | <1MB/day | 5 req/min, 25/day | **Good** | Free tier adequate |
| Google Trends | <1MB/day | Unofficial, varies | **Good** | Use pytrends |
| Baltic Dry Index | <1KB/day | N/A (scrape) | **Excellent** | Trivial data |
| Reddit API | 1-10MB/day | 100 req/min | **Good** | Use PRAW library |
| Satellite Imagery | 100MB-1GB/day | N/A | **Poor** | Too large, no free API |
| Credit Card Data | N/A | N/A | **Impossible** | Institutional only |

### 5.3 Storage Requirements

| Data Type | Storage per Year | 5-Year Archive |
|-----------|------------------|----------------|
| FRED economic series (50 indicators) | ~50MB | 250MB |
| SEC filings (parsed text, 500 companies) | ~2GB | 10GB |
| Google Trends (100 keywords) | ~100MB | 500MB |
| Reddit mentions (top 100 tickers) | ~500MB | 2.5GB |
| Baltic Dry Index | <1MB | <5MB |
| **Total Estimated** | **~3GB/year** | **~15GB** |

A 128GB microSD or USB SSD is more than adequate.

### 5.4 Processing Considerations

**Suitable for Pi:**
- Time series aggregation and smoothing
- Simple statistical correlations
- Keyword extraction and counting
- Rule-based signal generation
- SQLite/TimescaleDB queries

**May Require Cloud/Offload:**
- Large language model sentiment analysis
- Computer vision (satellite image analysis)
- Deep learning model training
- Real-time streaming of high-volume data

### 5.5 Recommended Architecture

```
[External APIs] --> [NDP Bronze Layer (Parquet)]
                          |
                          v
              [NDP Silver Layer (TimescaleDB)]
                          |
                          v
              [NDP Gold Layer (Feature Aggregations)]
                          |
                          v
              [Signal Generation (Rules Engine)]
                          |
                          v
              [Dashboard/Alerts (Grafana)]
```

All processing happens on-Pi with daily batch updates. No real-time streaming required for long-term investing signals.

---

## 6. Recommendations for NDP Integration

### 6.1 Priority Data Sources (Implement First)

#### Tier 1: High Value, Free, Pi-Friendly

| Source | Signal Type | Update Frequency | Implementation Effort |
|--------|-------------|------------------|----------------------|
| **FRED Leading Indicators** | Risk-on/off, Recession | Daily/Weekly | Low |
| **Baltic Dry Index** | Global trade health | Daily | Low |
| **SEC EDGAR 8-K** | Corporate events | Real-time polling | Medium |
| **Google Trends** | Retail sentiment | Weekly | Low |
| **Yield Curve Spread** | Recession probability | Daily | Low (via FRED) |

#### Tier 2: Moderate Value, Free/Low-Cost

| Source | Signal Type | Update Frequency | Implementation Effort |
|--------|-------------|------------------|----------------------|
| **Reddit Mentions** | Retail attention (contrarian) | Hourly | Medium |
| **USPTO Patents** | Innovation trends | Monthly | Medium |
| **IMF/World Bank** | Global macro | Monthly/Quarterly | Low |
| **Indeed Job Postings** | Economic health | Monthly (reports) | Low |

### 6.2 Proposed Bronze Streams

```yaml
# New alternative data streams for NDP

streams:
  - id: fred-leading-indicators
    source: api
    endpoint: https://api.stlouisfed.org/fred/
    frequency: daily
    series:
      - T10Y2Y  # Yield curve
      - ICSA    # Initial claims
      - PERMIT  # Building permits
      - UMCSENT # Consumer sentiment
      - USALOLITONOSTSAM  # OECD CLI

  - id: baltic-dry-index
    source: scrape
    url: https://tradingeconomics.com/commodity/baltic
    frequency: daily

  - id: google-trends-finance
    source: api
    library: pytrends
    keywords:
      - "stock market crash"
      - "recession"
      - "buy stocks"
      - "sell stocks"
      - "unemployment"
    frequency: weekly

  - id: reddit-stock-mentions
    source: api
    endpoint: https://apewisdom.io/api/
    frequency: hourly

  - id: sec-edgar-8k
    source: api
    endpoint: https://data.sec.gov/
    watchlist: sp500
    frequency: polling (15 min)
```

### 6.3 Proposed Silver Tables

```sql
-- Leading Economic Indicators
CREATE TABLE silver.economic_indicators (
    timestamp TIMESTAMPTZ NOT NULL,
    indicator_id TEXT NOT NULL,
    value DOUBLE PRECISION,
    mom_change DOUBLE PRECISION,  -- Month-over-month
    yoy_change DOUBLE PRECISION,  -- Year-over-year
    z_score DOUBLE PRECISION,     -- Standardized
    signal TEXT,                  -- BULLISH/BEARISH/NEUTRAL
    PRIMARY KEY (timestamp, indicator_id)
);

-- Sentiment Aggregates
CREATE TABLE silver.sentiment_daily (
    timestamp TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,          -- google_trends, reddit, etc.
    metric TEXT NOT NULL,          -- search_volume, mentions, sentiment
    value DOUBLE PRECISION,
    percentile DOUBLE PRECISION,   -- Historical percentile
    PRIMARY KEY (timestamp, source, metric)
);

-- Composite Signals
CREATE TABLE silver.alternative_data_signals (
    timestamp TIMESTAMPTZ NOT NULL,
    signal_name TEXT NOT NULL,
    signal_value DOUBLE PRECISION, -- -1 to +1 scale
    confidence DOUBLE PRECISION,
    components JSONB,              -- Contributing factors
    PRIMARY KEY (timestamp, signal_name)
);
```

### 6.4 Signal Generation Logic

**Risk-On/Risk-Off Composite:**
```python
def calculate_risk_signal():
    signals = {
        'yield_curve': yield_spread_z_score() * 0.25,
        'bdi': bdi_momentum() * 0.15,
        'claims': -initial_claims_z_score() * 0.20,
        'cli': cli_momentum() * 0.20,
        'sentiment': -fear_search_z_score() * 0.10,  # Contrarian
        'reddit_contrarian': -wsb_attention_z_score() * 0.10
    }
    return sum(signals.values())  # -1 (risk-off) to +1 (risk-on)
```

### 6.5 Implementation Roadmap

| Phase | Scope | Duration | Deliverables |
|-------|-------|----------|--------------|
| **1** | FRED integration | 1 week | Bronze stream, Silver table, basic signals |
| **2** | BDI + Yield curve | 3 days | Scraper, recession probability signal |
| **3** | Google Trends | 1 week | pytrends integration, keyword tracking |
| **4** | Reddit sentiment | 1 week | ApeWisdom API, contrarian signal |
| **5** | SEC EDGAR | 2 weeks | 8-K parser, event detection |
| **6** | Composite signals | 1 week | Risk-on/off dashboard, alerts |

### 6.6 What NOT to Pursue

**Skip These for NDP:**
1. **Satellite imagery** - Too expensive, too much data, no free options
2. **Credit card data** - Institutional only, impossible to access
3. **Real-time foot traffic** - Paid subscriptions only, marginal value
4. **High-frequency social sentiment** - Signal decay too fast for long-term
5. **Web traffic data (SimilarWeb)** - Free tier too limited

---

## References and Sources

### Research Papers
- [Eye in outer space: satellite imageries of container ports can predict world stock returns](https://www.nature.com/articles/s41599-023-01891-9) - Nature Humanities and Social Sciences Communications
- [How hedge funds use satellite images to beat Wall Street](https://newsroom.haas.berkeley.edu/how-hedge-funds-use-satellite-images-to-beat-wall-street-and-main-street/) - UC Berkeley Haas
- [An empirical investigation of forward-looking retailer performance using parking lot traffic data](https://www.sciencedirect.com/science/article/abs/pii/S0022435922000240) - ScienceDirect
- [WallStreetBets: Assessing the Collective Intelligence of Reddit](https://dl.acm.org/doi/10.1145/3660760) - ACM Transactions
- [Social media attention and retail investor behavior: Evidence from r/wallstreetbets](https://www.sciencedirect.com/science/article/pii/S1057521924006537) - ScienceDirect
- [Google search trends and stock markets: Sentiment, attention or uncertainty?](https://www.sciencedirect.com/science/article/pii/S1057521923000650) - ScienceDirect
- [Baltic dry index forecast using financial market data](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0325106) - PLOS One
- [Mispriced Innovation - Patents as a Leading Indicator for Earnings Growth](https://www.osam.com/Commentary/mispriced-innovation) - O'Shaughnessy Asset Management

### Data Sources
- [FRED - Federal Reserve Economic Data](https://fred.stlouisfed.org)
- [SEC EDGAR APIs](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)
- [IMF Data](https://data.imf.org)
- [World Bank Open Data](https://data.worldbank.org)
- [OECD Data Explorer](https://data-explorer.oecd.org)
- [Alpha Vantage](https://www.alphavantage.co)
- [Finnhub](https://finnhub.io)
- [ApeWisdom API](https://apewisdom.io/api/)
- [Baltic Dry Index - TradingEconomics](https://tradingeconomics.com/commodity/baltic)
- [USPTO Research Datasets](https://www.uspto.gov/ip-policy/economic-research/research-datasets)
- [LinkedIn Economic Graph](https://economicgraph.linkedin.com/workforce-data)

### Legal and Compliance
- [Web Scraping Legal Issues: 2025 Enterprise Compliance Guide](https://groupbwt.com/blog/is-web-scraping-legal/)
- [The SEC, Web Scraping, and Material Non-Public Information](https://mccarthylg.com/sec-puts-web-scraping-and-the-investment-firms-who-use-it-in-the-crosshairs/)

### Alternative Data Market
- [Alternative Data Market Size & Growth](https://www.grandviewresearch.com/industry-analysis/alternative-data-market)
- [How Alternative Data Is Transforming Investment Decisions in 2025](https://altindex.com/news/alternative-data-transforming-investment-decisions)

---

## Appendix A: API Code Examples

### FRED API (Python)
```python
import requests

FRED_API_KEY = "your_api_key"  # Free at https://fred.stlouisfed.org/docs/api/api_key.html

def get_fred_series(series_id: str, start_date: str = "2020-01-01"):
    url = f"https://api.stlouisfed.org/fred/series/observations"
    params = {
        "series_id": series_id,
        "api_key": FRED_API_KEY,
        "file_type": "json",
        "observation_start": start_date
    }
    response = requests.get(url, params=params)
    return response.json()["observations"]

# Example: Get yield curve spread
yield_curve = get_fred_series("T10Y2Y")
```

### Google Trends (Python)
```python
from pytrends.request import TrendReq

pytrends = TrendReq(hl='en-US', tz=360)

keywords = ["stock market crash", "recession", "buy stocks"]
pytrends.build_payload(keywords, timeframe='today 3-m')

interest_over_time = pytrends.interest_over_time()
```

### ApeWisdom Reddit API
```python
import requests

def get_reddit_mentions(filter_type: str = "all-stocks"):
    url = f"https://apewisdom.io/api/v1.0/filter/{filter_type}"
    response = requests.get(url)
    return response.json()["results"]

# Get top mentioned stocks on Reddit
mentions = get_reddit_mentions()
for stock in mentions[:10]:
    print(f"{stock['ticker']}: {stock['mentions']} mentions")
```

### SEC EDGAR (Python with edgartools)
```python
from edgar import Company, set_identity

set_identity("Your Name your@email.com")

# Get Apple's latest 8-K filings
apple = Company("AAPL")
filings_8k = apple.get_filings(form="8-K").latest(10)

for filing in filings_8k:
    print(f"{filing.filing_date}: {filing.description}")
```

---

*Document Version: 1.0*
*Last Updated: 2026-02-02*
*Author: NDP Research Agent*
