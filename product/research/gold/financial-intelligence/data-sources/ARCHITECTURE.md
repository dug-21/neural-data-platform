# Data Architecture for Financial Intelligence

**Research Date**: 2026-02-02
**Platform**: Raspberry Pi 5 (ARM64, 8GB RAM)
**Context**: Long-term investing, daily data cadence, edge deployment
**Budget**: Budget-conscious (free/low-cost data sources)

---

## Executive Summary

This document defines a data architecture for integrating financial intelligence into the Neural Data Platform (NDP). The architecture extends NDP's proven Bronze/Silver/Gold medallion pattern to handle diverse financial data sources including time series (prices, indicators), events (corporate actions, economic releases), sentiment (news, social), and alternative data.

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Daily cadence primary** | Long-term investing doesn't require intraday; reduces API costs and storage |
| **Free-tier APIs first** | Budget-conscious; upgrade path available if needed |
| **Unified Bronze schema** | All sources normalized to `timestamp + ndp_id + raw_payload` pattern |
| **Survivorship-aware design** | Track delisted assets; avoid backtest bias |
| **Edge-first computation** | Aggregations run on Pi; minimize cloud dependency |

---

## 1. Data Source Catalog

### 1.1 Free Tier API Comparison

| Provider | Data Types | Free Tier Limits | Lag | Historical Depth | Best For |
|----------|-----------|------------------|-----|------------------|----------|
| **Alpha Vantage** | Stocks, Forex, Crypto, Technical Indicators | 25 req/day (some sources say 500/day, 5/min) | Real-time | 20+ years | Primary price data |
| **Alpaca** | US Stocks, Crypto, Options | 10K req/min (with account) | 15-min delayed (free) / Real-time (Pro) | 7+ years | Best free historical |
| **Polygon.io** | US Stocks | 5 req/min (free) | EOD only | Limited | Secondary validation |
| **FRED** | Economic Indicators | Unlimited (with API key) | Same-day | Varies (some 1926+) | Macro data (primary) |
| **Nasdaq Data Link** | Mixed (200+ datasets) | Limited concurrent calls | Varies | Varies | Alternative datasets |
| **Finnhub** | Stocks, Forex, Crypto, News | 60 req/min | Mixed | Limited | News/sentiment |
| **Yahoo Finance** | Stocks, ETFs, Mutual Funds | Unofficial (no guarantees) | 15-min delayed | 20+ years | Backup/validation |
| **MarketStack** | Global Stocks | 100 req/month | EOD | 30+ years | International coverage |

### 1.2 Data Source Selection Matrix

For a long-term investing use case on Raspberry Pi:

| Data Need | Primary Source | Backup Source | Cadence |
|-----------|---------------|---------------|---------|
| US Stock Prices | Alpaca | Alpha Vantage | Daily |
| Fundamental Data | Alpha Vantage | Yahoo Finance | Quarterly |
| Economic Indicators | FRED | Nasdaq Data Link | As released |
| News Sentiment | Finnhub | Alpha Vantage | Daily |
| Corporate Events | Alpha Vantage | Yahoo Finance | As announced |
| Technical Indicators | Computed locally | Alpha Vantage | Daily |
| Index Constituents | Wikipedia scrape + validation | ETF holdings | Monthly |

### 1.3 API Configuration Examples

#### Alpha Vantage Stream Configuration

```yaml
stream_id: stock-daily-prices
description: "Daily OHLCV data from Alpha Vantage"
version: "1.0.0"
enabled: true
retention_days: 3650  # 10 years
compression_after_days: 7
partitioning_strategy: daily

sources:
  - type: http_poll
    enabled: true
    ndp_id: "fin-alphavantage-daily"
    context:
      source_type:
        provider: alphavantage
        purpose: daily_adjusted
      data_domain: financial
      asset_class: equity
    poll_interval_secs: 86400  # Once per day
    timeout_secs: 60
    parser_name: alphavantage_daily_adjusted
    endpoints:
      - endpoint_id: alphavantage_spy
        symbol: SPY
        url: "https://www.alphavantage.co/query?function=TIME_SERIES_DAILY_ADJUSTED&symbol=SPY&outputsize=compact"
        auth_type: query_param
        auth_key: apikey
        auth_value: "${ALPHAVANTAGE_API_KEY}"
    parser:
      parser_type: json_path
      field_mappings:
        - path: "Time Series (Daily).[date].1. open"
          metric_name: open
          unit: usd
        - path: "Time Series (Daily).[date].2. high"
          metric_name: high
          unit: usd
        - path: "Time Series (Daily).[date].3. low"
          metric_name: low
          unit: usd
        - path: "Time Series (Daily).[date].4. close"
          metric_name: close
          unit: usd
        - path: "Time Series (Daily).[date].5. adjusted close"
          metric_name: adjusted_close
          unit: usd
        - path: "Time Series (Daily).[date].6. volume"
          metric_name: volume
          unit: shares
        - path: "Time Series (Daily).[date].7. dividend amount"
          metric_name: dividend
          unit: usd
        - path: "Time Series (Daily).[date].8. split coefficient"
          metric_name: split_coefficient
          unit: ratio
```

#### FRED Economic Data Configuration

```yaml
stream_id: economic-indicators
description: "Federal Reserve Economic Data (FRED)"
version: "1.0.0"
enabled: true
retention_days: 7300  # 20 years
compression_after_days: 30
partitioning_strategy: monthly

sources:
  - type: http_poll
    enabled: true
    ndp_id: "fin-fred-macro"
    context:
      source_type:
        provider: fred
        purpose: economic_indicators
      data_domain: macro
    poll_interval_secs: 86400  # Daily check
    timeout_secs: 30
    parser_name: fred_observations
    endpoints:
      # GDP (Quarterly)
      - endpoint_id: fred_gdp
        series_id: GDP
        url: "https://api.stlouisfed.org/fred/series/observations?series_id=GDP&file_type=json"
        auth_type: query_param
        auth_key: api_key
        auth_value: "${FRED_API_KEY}"
      # Unemployment Rate (Monthly)
      - endpoint_id: fred_unemployment
        series_id: UNRATE
        url: "https://api.stlouisfed.org/fred/series/observations?series_id=UNRATE&file_type=json"
        auth_type: query_param
        auth_key: api_key
        auth_value: "${FRED_API_KEY}"
      # CPI (Monthly)
      - endpoint_id: fred_cpi
        series_id: CPIAUCSL
        url: "https://api.stlouisfed.org/fred/series/observations?series_id=CPIAUCSL&file_type=json"
        auth_type: query_param
        auth_key: api_key
        auth_value: "${FRED_API_KEY}"
      # Federal Funds Rate
      - endpoint_id: fred_fedfunds
        series_id: FEDFUNDS
        url: "https://api.stlouisfed.org/fred/series/observations?series_id=FEDFUNDS&file_type=json"
        auth_type: query_param
        auth_key: api_key
        auth_value: "${FRED_API_KEY}"
      # 10-Year Treasury
      - endpoint_id: fred_gs10
        series_id: GS10
        url: "https://api.stlouisfed.org/fred/series/observations?series_id=GS10&file_type=json"
        auth_type: query_param
        auth_key: api_key
        auth_value: "${FRED_API_KEY}"
```

---

## 2. Schema Designs

### 2.1 Bronze Layer Schema (Unified)

All financial data enters Bronze with NDP's standard schema, preserving the full raw payload.

```
Bronze Schema: raw_financial_data
├── timestamp (BIGINT, microseconds) - Observation/quote timestamp
├── ingestion_time (BIGINT) - When NDP received it
├── ndp_id (TEXT) - Source identifier (e.g., "fin-alphavantage-spy")
├── context (JSONB)
│   ├── source_type.provider (TEXT)
│   ├── source_type.purpose (TEXT)
│   ├── data_domain (TEXT) - "equity", "macro", "sentiment"
│   ├── asset_class (TEXT) - "stock", "etf", "bond", "indicator"
│   └── location.symbol (TEXT) - Ticker or series ID
└── raw_payload (JSONB) - Full API response
```

### 2.2 Silver Layer Schemas

#### 2.2.1 Multi-Asset Time Series (Prices)

```sql
-- Silver: Normalized price observations
CREATE TABLE silver.price_observations (
    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,

    -- Asset identification
    symbol TEXT NOT NULL,
    asset_class TEXT NOT NULL,  -- 'stock', 'etf', 'crypto', 'forex'
    exchange TEXT,

    -- OHLCV data
    open_price DOUBLE PRECISION,
    high_price DOUBLE PRECISION,
    low_price DOUBLE PRECISION,
    close_price DOUBLE PRECISION,
    adjusted_close DOUBLE PRECISION,  -- Split/dividend adjusted
    volume BIGINT,

    -- Corporate actions (embedded)
    dividend_amount DOUBLE PRECISION,
    split_coefficient DOUBLE PRECISION,

    -- Data quality
    dq_flags TEXT[],

    PRIMARY KEY (observation_time, symbol)
);

-- Convert to TimescaleDB hypertable
SELECT create_hypertable('silver.price_observations', 'observation_time',
    chunk_time_interval => INTERVAL '1 month');

-- Index for symbol queries
CREATE INDEX idx_price_symbol ON silver.price_observations (symbol, observation_time DESC);
```

#### 2.2.2 Event Schema (Corporate Actions, Economic Releases)

```sql
-- Silver: Discrete events
CREATE TABLE silver.financial_events (
    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,

    -- Event classification
    event_type TEXT NOT NULL,  -- 'earnings', 'dividend', 'split', 'economic_release', 'ipo', 'delisting'
    event_subtype TEXT,        -- 'beat', 'miss', 'in-line' for earnings

    -- Asset reference (nullable for macro events)
    symbol TEXT,
    asset_class TEXT,

    -- Event details
    event_value DOUBLE PRECISION,      -- The numeric value (e.g., EPS, dividend amount)
    event_value_unit TEXT,             -- 'usd', 'percent', 'ratio'
    expected_value DOUBLE PRECISION,   -- Consensus/expected (for surprises)
    previous_value DOUBLE PRECISION,   -- Previous period's value

    -- Magnitude/impact
    surprise_pct DOUBLE PRECISION,     -- (actual - expected) / expected
    impact_score DOUBLE PRECISION,     -- Computed significance (0-1)

    -- Metadata
    source_provider TEXT NOT NULL,
    event_description TEXT,

    -- Data quality
    dq_flags TEXT[],

    PRIMARY KEY (event_time, event_type, COALESCE(symbol, ''))
);

SELECT create_hypertable('silver.financial_events', 'event_time',
    chunk_time_interval => INTERVAL '1 month');
```

#### 2.2.3 Sentiment Schema

```sql
-- Silver: Sentiment scores
CREATE TABLE silver.sentiment_scores (
    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,

    -- Subject
    symbol TEXT,                  -- Can be NULL for market-wide sentiment
    sector TEXT,                  -- GICS sector if applicable

    -- Sentiment metrics
    sentiment_score DOUBLE PRECISION NOT NULL,  -- -1 to +1 normalized
    sentiment_magnitude DOUBLE PRECISION,       -- Strength (0-1)
    confidence DOUBLE PRECISION,                -- Model confidence (0-1)

    -- Source details
    source_type TEXT NOT NULL,    -- 'news', 'social', 'analyst', 'options_flow'
    source_provider TEXT NOT NULL,
    article_count INTEGER,        -- Number of articles/mentions aggregated

    -- Metadata
    dominant_topics TEXT[],       -- Key topics identified

    -- Data quality
    dq_flags TEXT[],

    PRIMARY KEY (observation_time, source_type, COALESCE(symbol, ''))
);

SELECT create_hypertable('silver.sentiment_scores', 'observation_time',
    chunk_time_interval => INTERVAL '1 month');
```

#### 2.2.4 Economic Indicators Schema

```sql
-- Silver: Economic indicator observations
CREATE TABLE silver.economic_indicators (
    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,

    -- Indicator identification
    series_id TEXT NOT NULL,       -- FRED series ID (e.g., 'GDP', 'UNRATE')
    indicator_name TEXT NOT NULL,
    category TEXT,                 -- 'growth', 'employment', 'inflation', 'rates'

    -- Values
    value DOUBLE PRECISION NOT NULL,
    value_unit TEXT,               -- 'percent', 'billions_usd', 'index'

    -- Context
    frequency TEXT,                -- 'daily', 'weekly', 'monthly', 'quarterly'
    seasonal_adjustment TEXT,      -- 'SA', 'NSA', 'SAAR'

    -- Revisions tracking (point-in-time)
    revision_number INTEGER DEFAULT 0,
    original_release_date DATE,
    is_preliminary BOOLEAN DEFAULT FALSE,

    -- Data quality
    dq_flags TEXT[],

    PRIMARY KEY (observation_time, series_id, revision_number)
);

SELECT create_hypertable('silver.economic_indicators', 'observation_time',
    chunk_time_interval => INTERVAL '1 year');
```

#### 2.2.5 Asset Universe Schema (Survivorship Tracking)

```sql
-- Silver: Asset universe tracking (handles survivorship bias)
CREATE TABLE silver.asset_universe (
    effective_date DATE NOT NULL,
    symbol TEXT NOT NULL,

    -- Asset details
    company_name TEXT,
    asset_class TEXT NOT NULL,
    exchange TEXT,
    sector TEXT,
    industry TEXT,
    market_cap_bucket TEXT,        -- 'mega', 'large', 'mid', 'small', 'micro'

    -- Status
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    listing_date DATE,
    delisting_date DATE,
    delisting_reason TEXT,         -- 'merger', 'acquisition', 'bankruptcy', 'voluntary', 'exchange_move'

    -- Index membership (point-in-time)
    sp500_member BOOLEAN DEFAULT FALSE,
    russell1000_member BOOLEAN DEFAULT FALSE,
    russell2000_member BOOLEAN DEFAULT FALSE,

    PRIMARY KEY (effective_date, symbol)
);

-- Create index for current universe queries
CREATE INDEX idx_universe_active ON silver.asset_universe (symbol, effective_date DESC)
WHERE is_active = TRUE;

-- Create index for historical queries
CREATE INDEX idx_universe_date ON silver.asset_universe (effective_date, symbol);
```

### 2.3 Gold Layer Schemas

#### 2.3.1 Feature Store for ML

```sql
-- Gold: Pre-computed features for ML models
CREATE TABLE gold.price_features (
    feature_time TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,

    -- Returns
    return_1d DOUBLE PRECISION,        -- Daily return
    return_5d DOUBLE PRECISION,        -- 5-day return
    return_21d DOUBLE PRECISION,       -- Monthly return
    return_63d DOUBLE PRECISION,       -- Quarterly return
    return_252d DOUBLE PRECISION,      -- Annual return

    -- Volatility
    volatility_21d DOUBLE PRECISION,   -- 21-day rolling volatility
    volatility_63d DOUBLE PRECISION,   -- 63-day rolling volatility

    -- Momentum
    rsi_14 DOUBLE PRECISION,           -- 14-day RSI
    macd_signal DOUBLE PRECISION,      -- MACD histogram
    price_vs_sma_50 DOUBLE PRECISION,  -- % above/below 50-day SMA
    price_vs_sma_200 DOUBLE PRECISION, -- % above/below 200-day SMA

    -- Volume
    volume_ratio_21d DOUBLE PRECISION, -- Volume vs 21-day average

    -- Valuation (from fundamentals)
    pe_ratio DOUBLE PRECISION,
    pb_ratio DOUBLE PRECISION,
    dividend_yield DOUBLE PRECISION,

    -- Macro context
    yield_curve_slope DOUBLE PRECISION, -- 10Y - 2Y spread
    vix_level DOUBLE PRECISION,

    PRIMARY KEY (feature_time, symbol)
);

-- TimescaleDB continuous aggregate for automated refresh
CREATE MATERIALIZED VIEW gold.price_features_daily
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 day', observation_time) AS feature_time,
    symbol,

    -- Close prices for return calculations
    LAST(adjusted_close, observation_time) AS close_price,

    -- Volume aggregation
    SUM(volume) AS total_volume,
    AVG(volume) AS avg_volume

FROM silver.price_observations
GROUP BY feature_time, symbol;
```

---

## 3. Bronze/Silver/Gold Mapping

### 3.1 Data Flow Diagram

```
                    ┌─────────────────────────────────────────────────────────────┐
                    │                      DATA SOURCES                           │
                    │                                                             │
                    │  Alpha Vantage   Alpaca   FRED   Finnhub   Yahoo Finance   │
                    └──────┬──────────────┬────────┬──────────┬──────────────────┘
                           │              │        │          │
                           ▼              ▼        ▼          ▼
┌──────────────────────────────────────────────────────────────────────────────────┐
│                                BRONZE LAYER                                       │
│                                                                                   │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌────────────────┐   │
│  │ stock-daily-   │ │ economic-      │ │ news-sentiment │ │ corporate-     │   │
│  │ prices/        │ │ indicators/    │ │ /              │ │ events/        │   │
│  │ *.parquet      │ │ *.parquet      │ │ *.parquet      │ │ *.parquet      │   │
│  └────────────────┘ └────────────────┘ └────────────────┘ └────────────────┘   │
│                                                                                   │
│  Schema: timestamp | ndp_id | context | raw_payload                             │
└──────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        │ ETL (config-driven)
                                        ▼
┌──────────────────────────────────────────────────────────────────────────────────┐
│                                SILVER LAYER                                       │
│                              (TimescaleDB)                                       │
│                                                                                   │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐                │
│  │ price_          │ │ economic_        │ │ sentiment_       │                │
│  │ observations    │ │ indicators       │ │ scores           │                │
│  ├──────────────────┤ ├──────────────────┤ ├──────────────────┤                │
│  │ - Normalized    │ │ - Point-in-time │ │ - Normalized     │                │
│  │ - DQ validated  │ │ - Revisions     │ │ - Source tracked │                │
│  │ - Adjusted      │ │ - Categorized   │ │ - Confidence     │                │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘                │
│                                                                                   │
│  ┌──────────────────┐ ┌──────────────────┐                                     │
│  │ financial_      │ │ asset_universe   │                                     │
│  │ events          │ │ (survivorship)   │                                     │
│  └──────────────────┘ └──────────────────┘                                     │
└──────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        │ Continuous Aggregates
                                        ▼
┌──────────────────────────────────────────────────────────────────────────────────┐
│                                GOLD LAYER                                         │
│                       (ML Features + Analytics)                                  │
│                                                                                   │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │                         price_features                                    │   │
│  │  - Returns (1d, 5d, 21d, 63d, 252d)                                     │   │
│  │  - Volatility (21d, 63d rolling)                                        │   │
│  │  - Momentum (RSI, MACD, SMA ratios)                                     │   │
│  │  - Valuation (P/E, P/B, dividend yield)                                 │   │
│  └──────────────────────────────────────────────────────────────────────────┘   │
│                                                                                   │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │                        macro_context                                      │   │
│  │  - Yield curve slope                                                     │   │
│  │  - Credit spreads                                                        │   │
│  │  - Economic surprise index                                               │   │
│  │  - Sentiment aggregates                                                  │   │
│  └──────────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 ETL Configuration

```yaml
# Silver ETL for price data
silver_etl:
  enabled: true
  target_table: silver.price_observations
  description: "Daily price observations from multiple sources"
  grain: "One row per symbol per trading day"

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id
    - source: context.location.symbol
      target: symbol

  field_mappings:
    - source_path: raw_payload.open
      target_column: open_price
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100000.0
          action: flag

    - source_path: raw_payload.high
      target_column: high_price
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100000.0
          action: flag
        - rule: cross_field_check
          expression: "high_price >= open_price AND high_price >= close_price"
          action: flag

    - source_path: raw_payload.adjusted_close
      target_column: adjusted_close
      type: double_precision
      nullable: false
      dq_rules:
        - rule: range_check
          min: 0.001
          max: 100000.0
          action: flag

    - source_path: raw_payload.volume
      target_column: volume
      type: bigint
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0
          max: 10000000000
          action: flag

  # Cross-field DQ rules
  dq_rules:
    - rule: cross_field_check
      name: high_low_valid
      expression: "high_price >= low_price"
      message: "high_below_low"
      action: flag

    - rule: cross_field_check
      name: ohlc_range_valid
      expression: "high_price >= open_price AND high_price >= close_price AND low_price <= open_price AND low_price <= close_price"
      message: "ohlc_range_violation"
      action: flag

  deduplication:
    enabled: true
    key_columns: [observation_time, symbol]
    strategy: upsert  # Latest source wins

  incremental:
    enabled: true
    watermark_column: observation_time
    lag_interval: 1 day
```

---

## 4. Data Quality Considerations

### 4.1 Survivorship Bias Mitigation

**The Problem:**
- Free data sources typically exclude delisted stocks
- Historical backtests on "current" universe are overly optimistic
- Missing ~75% of stocks that existed 10 years ago

**NDP Solution:**

```sql
-- Track universe changes monthly
INSERT INTO silver.asset_universe (effective_date, symbol, is_active, ...)
SELECT
    DATE_TRUNC('month', CURRENT_DATE) AS effective_date,
    symbol,
    TRUE AS is_active,
    ...
FROM current_holdings  -- From ETF holdings disclosure
ON CONFLICT (effective_date, symbol)
DO UPDATE SET is_active = EXCLUDED.is_active;

-- Mark delisted securities
UPDATE silver.asset_universe
SET
    is_active = FALSE,
    delisting_date = CURRENT_DATE,
    delisting_reason = 'detected_missing'
WHERE symbol NOT IN (SELECT symbol FROM latest_active_universe)
AND is_active = TRUE;
```

**Free Survivorship-Bias-Free Sources:**
- Wikipedia S&P 500 historical revisions
- IVV/SPY ETF monthly holdings disclosures
- Historical constituent lists from financial bloggers

### 4.2 Point-in-Time Data (Avoiding Look-Ahead Bias)

**The Problem:**
- Economic data is revised multiple times
- Fundamentals are restated
- Using final values in backtests is cheating

**NDP Solution:**

```yaml
# Track revisions in Silver
silver.economic_indicators:
  - revision_number: 0 (preliminary)
  - revision_number: 1 (first revision)
  - revision_number: 2 (final)
  - original_release_date: When first available
```

```sql
-- Point-in-time query: What was known on a specific date?
SELECT * FROM silver.economic_indicators
WHERE series_id = 'GDP'
AND observation_time <= '2025-06-30'  -- Data period
AND original_release_date <= '2025-07-30'  -- Available by this date
ORDER BY observation_time DESC, revision_number ASC
LIMIT 1;
```

### 4.3 Corporate Actions Handling

| Action | Challenge | NDP Approach |
|--------|-----------|--------------|
| **Stock Splits** | Historical prices need adjustment | Use `adjusted_close`; store `split_coefficient` |
| **Dividends** | Total return requires reinvestment | Use `adjusted_close`; store `dividend_amount` |
| **Spinoffs** | Creates new ticker, affects parent | Track in `financial_events`; manual review |
| **Mergers** | One ticker disappears | Mark delisting; link to acquirer |
| **Symbol Changes** | Same company, new ticker | Create mapping table |

```sql
-- Symbol mapping for continuity
CREATE TABLE silver.symbol_mapping (
    old_symbol TEXT NOT NULL,
    new_symbol TEXT NOT NULL,
    effective_date DATE NOT NULL,
    change_type TEXT NOT NULL,  -- 'rename', 'merger', 'spinoff'
    PRIMARY KEY (old_symbol, effective_date)
);
```

### 4.4 Missing Data Handling

```yaml
# DQ rules for missing data
dq_rules:
  # Flag extended gaps
  - rule: gap_detection
    field: observation_time
    max_gap: 5 days  # Allow weekends + holidays
    action: flag

  # Interpolation for technical indicators
  - rule: interpolation
    method: linear
    max_gap: 3  # Only interpolate up to 3 missing days
    fields: [close_price, volume]

  # Forward-fill for fundamentals (valid until next report)
  - rule: forward_fill
    fields: [pe_ratio, book_value]
    max_days: 120  # Quarterly data
```

---

## 5. Data Freshness Requirements

### 5.1 Update Cadence by Data Type

| Data Type | Update Frequency | Acceptable Lag | Rationale |
|-----------|------------------|----------------|-----------|
| **Daily Prices** | Once daily (after market close) | 1 hour | EOD sufficient for long-term |
| **Intraday Prices** | NOT NEEDED | N/A | Long-term focus |
| **Fundamentals** | Quarterly | 1 day | Quarterly reports |
| **Economic Indicators** | As released | Same day | Fed releases at 8:30 AM |
| **News Sentiment** | Daily aggregate | 24 hours | Batch processing OK |
| **Corporate Events** | As announced | Same day | Material events |
| **Index Constituents** | Monthly | 1 week | Gradual changes |

### 5.2 Scheduling Strategy

```
┌─────────────────────────────────────────────────────────────────────┐
│                     DAILY SCHEDULE (UTC)                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  21:30 UTC (4:30 PM ET)  │  US Markets Close                       │
│                          │                                          │
│  22:00 UTC               │  Poll Alpha Vantage/Alpaca for prices   │
│                          │  (Daily OHLCV for all symbols)          │
│                          │                                          │
│  22:30 UTC               │  Bronze → Silver ETL for prices         │
│                          │                                          │
│  23:00 UTC               │  Poll FRED for new economic releases    │
│                          │                                          │
│  23:30 UTC               │  Poll Finnhub for news sentiment        │
│                          │                                          │
│  00:00 UTC               │  Silver → Gold feature computation      │
│                          │  (Returns, volatility, indicators)      │
│                          │                                          │
│  00:30 UTC               │  Data validation and alerting           │
│                          │                                          │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 6. Storage Estimates

### 6.1 Per-Asset Storage

| Layer | Data Type | Size per Asset per Year |
|-------|-----------|-------------------------|
| Bronze (Parquet) | Daily prices | ~15 KB |
| Bronze (Parquet) | Fundamentals | ~5 KB |
| Silver (TimescaleDB) | Normalized prices | ~25 KB |
| Silver (TimescaleDB) | Events | ~2 KB |
| Gold (TimescaleDB) | Features | ~30 KB |

### 6.2 Total Storage Projections

**Assumptions:**
- 500 stocks in universe
- 10 years of history
- 100 economic indicators
- Daily sentiment scores

| Component | Calculation | Estimated Size |
|-----------|-------------|----------------|
| **Bronze (Prices)** | 500 assets x 10 years x 15 KB | 75 MB |
| **Bronze (Fundamentals)** | 500 assets x 10 years x 5 KB | 25 MB |
| **Bronze (Indicators)** | 100 series x 10 years x 2 KB | 2 MB |
| **Silver (Prices)** | 500 assets x 10 years x 25 KB | 125 MB |
| **Silver (Events)** | 500 assets x 10 events/year x 10 years x 0.5 KB | 25 MB |
| **Silver (Indicators)** | 100 series x 10 years x 5 KB | 5 MB |
| **Gold (Features)** | 500 assets x 10 years x 30 KB | 150 MB |
| **TimescaleDB Overhead** | ~30% of Silver+Gold | 90 MB |
| **Total** | | **~500 MB** |

**With Compression:**
- TimescaleDB compression: 80-95% reduction on historical
- Parquet columnar: ~60% reduction
- **Compressed Total: ~100-150 MB**

This is well within Raspberry Pi storage constraints.

### 6.3 Memory Requirements

| Component | Memory Usage |
|-----------|--------------|
| TimescaleDB (queries) | 256-512 MB |
| DuckDB (Bronze queries) | 256-512 MB |
| Feature computation | 128-256 MB |
| Application | 128-256 MB |
| **Total Peak** | **~1 GB** |

Fits comfortably on Pi 5 (8GB) with room for other workloads.

---

## 7. Edge Deployment Feasibility

### 7.1 What Runs on Pi

| Component | Pi Feasibility | Notes |
|-----------|---------------|-------|
| **API Polling** | Yes | Async Rust, minimal resources |
| **Bronze Storage** | Yes | Parquet + WAL, ~150 MB |
| **Silver ETL** | Yes | Config-driven, batch processing |
| **TimescaleDB** | Yes | 512 MB memory limit, hypertables |
| **Feature Computation** | Yes | SQL continuous aggregates |
| **ML Inference** | Yes | ruv-FANN, <100ms inference |
| **Backtesting** | Partial | Simple strategies OK; complex need cloud |

### 7.2 What Might Need Cloud/Desktop

| Task | Why | Recommendation |
|------|-----|----------------|
| **Initial Historical Load** | Large download, rate limits | Run on desktop, transfer Parquet to Pi |
| **Complex Backtests** | Memory-intensive Monte Carlo | Desktop or cloud notebook |
| **Model Training** | GPU acceleration useful | Desktop with GPU |
| **Alternative Data** | Satellite, web scraping | Cloud preprocessing |

### 7.3 Hybrid Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    RASPBERRY PI (EDGE)                          │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │ Daily API   │  │ Bronze/     │  │ ML          │            │
│  │ Polling     │  │ Silver ETL  │  │ Inference   │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
│         │                │                │                    │
│         ▼                ▼                ▼                    │
│  ┌──────────────────────────────────────────────┐             │
│  │              TimescaleDB / Parquet           │             │
│  │              (< 500 MB total)                │             │
│  └──────────────────────────────────────────────┘             │
│                          │                                     │
│                          │ Sync (optional)                     │
└──────────────────────────┼─────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    DESKTOP / CLOUD (OPTIONAL)                   │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │ Historical  │  │ Complex     │  │ Model       │            │
│  │ Data Load   │  │ Backtesting │  │ Training    │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 7.4 API Rate Limit Management

```rust
// Rate limiter for API calls
struct RateLimiter {
    requests_per_minute: u32,
    requests_per_day: u32,
    last_request: Instant,
    daily_count: AtomicU32,
    daily_reset: DateTime<Utc>,
}

impl RateLimiter {
    async fn acquire(&self) -> Result<(), RateLimitError> {
        // Check daily limit
        if self.daily_count.load(Ordering::Relaxed) >= self.requests_per_day {
            let wait_time = self.daily_reset - Utc::now();
            return Err(RateLimitError::DailyLimitReached {
                retry_after: wait_time
            });
        }

        // Check per-minute rate
        let elapsed = self.last_request.elapsed();
        let min_interval = Duration::from_secs(60) / self.requests_per_minute;
        if elapsed < min_interval {
            tokio::time::sleep(min_interval - elapsed).await;
        }

        self.daily_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

// Scheduling to stay within limits
struct DataIngestionScheduler {
    alpha_vantage: RateLimiter,  // 25/day
    fred: RateLimiter,           // Generous
    finnhub: RateLimiter,        // 60/min
}
```

### 7.5 Caching Strategy

```yaml
# Cache expensive API responses
cache:
  enabled: true
  backend: sqlite  # Lightweight for Pi
  path: /data/cache/api_cache.db

  rules:
    # Historical data: cache indefinitely
    - pattern: "alpha_vantage/daily/*"
      ttl: null  # Never expires (historical won't change)

    # Recent data: cache until next trading day
    - pattern: "alpha_vantage/daily/latest/*"
      ttl: 86400  # 24 hours

    # Economic releases: cache until next release
    - pattern: "fred/*"
      ttl: 604800  # 7 days

    # Sentiment: daily refresh
    - pattern: "finnhub/sentiment/*"
      ttl: 86400  # 24 hours
```

---

## 8. Cost Analysis

### 8.1 Free Tier Capacity

| Provider | Free Tier | Coverage Possible |
|----------|-----------|-------------------|
| **Alpha Vantage** | 25 req/day | 25 symbols daily OR 125 symbols weekly |
| **Alpaca** | Unlimited (with account) | Full US market |
| **FRED** | Unlimited | All economic indicators |
| **Finnhub** | 60 req/min | Full news coverage |
| **Yahoo Finance** | Unofficial | Backup only |

### 8.2 Recommended Free Stack

For a 50-symbol portfolio:

| Data Need | Provider | Daily Requests | Cost |
|-----------|----------|----------------|------|
| Daily prices (50 symbols) | Alpaca | 50 | $0 |
| Economic indicators (10) | FRED | 10 | $0 |
| News sentiment (50) | Finnhub | 50 | $0 |
| Validation sample | Alpha Vantage | 5 | $0 |
| **Total** | | 115 | **$0** |

### 8.3 Upgrade Path (If Needed)

| Upgrade | Cost/Month | Benefit |
|---------|------------|---------|
| Alpha Vantage Premium | $50 | 500 req/min, more data |
| Polygon.io Stocks Developer | $79 | Real-time, 10 years historical |
| Finnhub Professional | $19 | More news sources |
| Alpaca Algo Trader Plus | $99 | Real-time SIP data |

---

## 9. Implementation Recommendations

### 9.1 Phase 1: Foundation (Week 1-2)

1. **Create Bronze stream configs** for:
   - `stock-daily-prices` (Alpaca)
   - `economic-indicators` (FRED)

2. **Implement parsers** for:
   - Alpaca historical bars
   - FRED observations

3. **Setup Silver schemas** for:
   - `price_observations`
   - `economic_indicators`

### 9.2 Phase 2: Events & Sentiment (Week 3-4)

1. **Add Bronze streams** for:
   - `news-sentiment` (Finnhub)
   - `corporate-events` (Alpha Vantage)

2. **Create Silver schemas** for:
   - `sentiment_scores`
   - `financial_events`

3. **Implement survivorship tracking**

### 9.3 Phase 3: Gold Features (Week 5-6)

1. **Create continuous aggregates** for:
   - Price features (returns, volatility)
   - Technical indicators (RSI, MACD)

2. **Implement macro context** features

3. **Build feature validation** dashboard

### 9.4 Phase 4: ML Integration (Week 7-8)

1. **Connect to ruv-FANN** for inference
2. **Implement backtesting** framework (simple)
3. **Setup alerting** for data quality issues

---

## 10. References

### Internal Documentation
- [Platform Architecture Overview](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [Config-Driven Silver ETL Design](/workspaces/neural-data-platform/docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md)
- [Time-Series Feature Engineering](/workspaces/neural-data-platform/product/research/gold/feature-engineering/TIME-SERIES-FEATURES.md)
- [Financial Edge Applications](/workspaces/neural-data-platform/research/edgeplatform-realtime/domains/financial-edge.md)

### External API Documentation
- [Alpha Vantage API](https://www.alphavantage.co/documentation/)
- [Alpaca Market Data API](https://docs.alpaca.markets/docs/about-market-data-api)
- [FRED API](https://fred.stlouisfed.org/docs/api/fred/)
- [Finnhub API](https://finnhub.io/docs/api)
- [Nasdaq Data Link](https://data.nasdaq.com/)

### Data Quality References
- [Survivorship Bias in Backtesting](https://www.quantrocket.com/blog/survivorship-bias/)
- [Point-in-Time Data for Backtesting](https://www.luxalgo.com/blog/survivorship-bias-in-backtesting-explained/)
- [CRSP Survivorship-Bias-Free Data](https://www.crsp.org/products/documentation)

---

**Document Version**: 1.0
**Last Updated**: 2026-02-02
**Author**: Research Agent
**Status**: Complete
