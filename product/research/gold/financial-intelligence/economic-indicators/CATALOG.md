# Economic Indicators Catalog for Investment Decisions

> Comprehensive reference of macroeconomic indicators that predict market performance and economic turning points.

## Executive Summary

This catalog documents economic indicators with proven predictive power for investment decision-making. Indicators are classified by their lead/lag characteristics, historical accuracy, and sector-specific applications. All listed indicators are available through free or low-cost data sources with sufficient historical depth for backtesting.

**Key Findings:**
- Yield curve inversions have predicted every recession since 1955 with 87.5% accuracy
- Housing starts/building permits are the single most critical leading indicator according to Moody's machine learning research
- Multi-indicator models (combining 3-5 indicators) significantly outperform single-indicator approaches
- The Sahm Rule triggers during recessions with near-perfect accuracy but signals ~3 months after onset
- Credit spreads often lead stock market corrections by weeks or months

---

## Table of Contents

1. [Leading Indicators](#1-leading-indicators)
2. [Coincident Indicators](#2-coincident-indicators)
3. [Lagging Indicators](#3-lagging-indicators)
4. [Sector-Specific Indicators](#4-sector-specific-indicators)
5. [Multi-Indicator Models](#5-multi-indicator-models)
6. [Data Sources](#6-data-sources)
7. [Recommended Indicator Set for NDP](#7-recommended-indicator-set-for-ndp)
8. [Implementation Notes](#8-implementation-notes)

---

## 1. Leading Indicators

Leading indicators change direction **before** peaks or troughs in the business cycle, providing advance warning of economic turning points.

### 1.1 Conference Board Leading Economic Index (LEI)

| Attribute | Value |
|-----------|-------|
| **FRED Series** | USSLIND |
| **Frequency** | Monthly |
| **Lead Time** | ~7 months average |
| **Historical Accuracy** | High (with some false positives) |
| **Data Source** | Conference Board |

**Description:**
Composite of 10 leading indicators designed to anticipate turning points in the business cycle. Components include:
- Average weekly hours (manufacturing)
- Average weekly initial claims for unemployment insurance
- Manufacturers' new orders (consumer goods)
- ISM Index of New Orders
- Manufacturers' new orders (nondefense capital goods)
- Building permits (new private housing)
- Stock prices (S&P 500)
- Leading Credit Index
- Interest rate spread (10-year Treasury minus Fed funds)
- Average consumer expectations

**Predictive Performance:**
- Anticipates turning points by approximately 7 months
- Declined by 1.2% over six months (May-November 2025), a more moderate rate than prior periods
- Weak consumer expectations currently leading the decline

**Investment Signal:**
- 3+ consecutive monthly declines: Recession warning
- 6-month annualized change below -4%: Strong recession signal
- Rising trend: Economic expansion confirmation

**Limitations:**
- False positives occur (e.g., January 1996 with 1.8% decline but no recession)
- Components may move in conflicting directions

---

### 1.2 Yield Curve (Treasury Spread)

| Attribute | Value |
|-----------|-------|
| **FRED Series** | T10Y2Y (10Y-2Y), T10Y3M (10Y-3M) |
| **Frequency** | Daily |
| **Lead Time** | 6-24 months (avg 15 months) |
| **Historical Accuracy** | 87.5% (since 1955) |
| **Data Source** | Federal Reserve |

**Description:**
The spread between long-term and short-term Treasury yields. An inverted yield curve (negative spread) has historically preceded every recession since 1955 with one exception in 1966.

**Key Metrics:**
- **10Y-2Y Spread**: Most commonly cited
- **10Y-3M Spread**: Used by New York Fed for recession probability
- **Duration of Inversion**: Inversions >3 months show 73% accuracy vs 45% for <3 months

**Historical Performance:**
| Inversion Period | Recession Start | Lead Time |
|------------------|-----------------|-----------|
| August 2019 | March 2020 | 7 months |
| July 2006 | December 2007 | 17 months |
| February 2000 | March 2001 | 13 months |
| May 1998 | No recession | False positive |

**Investment Signal:**
- Inversion lasting >3 months: High recession probability (12-24 months)
- Deeper inversions (>100 bps): More severe recessions historically
- Steepening after inversion: Recession often begins soon after normalization

**Current Status (Q4 2025):**
- The 2022-2023 inversion lasted 16 months (longest in modern history)
- Normalized to +0.55% by October 2025
- No recession materialized despite extended inversion, challenging traditional interpretation

**Limitations:**
- Quantitative easing may distort long-term rates
- False positive risk in near-zero rate environments
- International capital flows affect Treasury demand

---

### 1.3 ISM Manufacturing PMI

| Attribute | Value |
|-----------|-------|
| **FRED Series** | NAPM (composite), NAPMNOI (new orders) |
| **Frequency** | Monthly (1st business day) |
| **Lead Time** | 1-3 months |
| **Historical Accuracy** | High for cyclical turns |
| **Data Source** | Institute for Supply Management |

**Description:**
Survey-based index measuring manufacturing sector health. Based on responses from 400+ industrial company purchasing executives.

**Key Thresholds:**
- **>50**: Manufacturing expansion
- **<50**: Manufacturing contraction
- **<42.6**: Economy-wide recession likely

**Sub-Indices:**
- **New Orders**: Leading indicator within PMI
- **Production**: Coincident indicator
- **Employment**: Lagging indicator
- **Prices Paid**: Inflation signal

**Investment Signal:**
- PMI rising above 50: Overweight cyclical sectors (industrials, financials, energy)
- PMI falling below 50: Defensive positioning (utilities, consumer staples)
- 3 consecutive months improving: Bullish for industrial stocks

**Sector Impact:**
| PMI Direction | Favored Sectors |
|---------------|-----------------|
| Rising >50 | Industrials, Materials, Financials |
| Falling <50 | Utilities, Consumer Staples, Healthcare |
| New Orders diverging from Production | Mean-reversion opportunities in commodities |

---

### 1.4 Building Permits and Housing Starts

| Attribute | Value |
|-----------|-------|
| **FRED Series** | PERMIT (permits), HOUST (housing starts) |
| **Frequency** | Monthly |
| **Lead Time** | 3-6 months for housing, 6-12 months for economy |
| **Historical Accuracy** | Very high (8 of 9 recessions preceded by housing decline) |
| **Data Source** | Census Bureau |

**Description:**
Moody's Analytics research using machine learning identified building permits as the **single most critical economic variable** for predicting U.S. recessions.

**Key Thresholds:**
- **<900k units (annualized)**: Recession warning level
- **500k units**: Crisis level (reached in 2008 GFC)
- **>1.5M units**: Strong expansion

**Predictive Performance:**
- 8 of last 9 recessions preceded by plunge in housing starts
- Construction activity leads most other economic production
- Early indicator of consumer/business confidence

**Investment Signal:**
- Permits declining 20%+ YoY: Recession risk elevated
- Permits rising: Homebuilders, banks, building materials favored
- Multi-family vs single-family divergence: Signals shifting demand

**Affected Sectors:**
- Homebuilders (DJUSHB index)
- Mortgage lenders and banks
- Building materials suppliers
- Raw materials (lumber, copper)

---

### 1.5 Initial Unemployment Claims

| Attribute | Value |
|-----------|-------|
| **FRED Series** | ICSA (weekly), ICLAIMSNS (not seasonally adjusted) |
| **Frequency** | Weekly (Thursday) |
| **Lead Time** | 3-22 months (avg 11 months from trough) |
| **Historical Accuracy** | Strong (near-coincident for recession end) |
| **Data Source** | Department of Labor |

**Description:**
High-frequency labor market indicator measuring new unemployment insurance filings.

**Key Thresholds:**
- **<400,000**: Healthy labor market
- **>434,165**: Deteriorating conditions signal (St. Louis Fed research)
- **Historical average**: 363,000 (since 1967)
- **Recent range**: ~225,000 (near all-time lows)

**Ratio Analysis:**
- Initial claims as % of Civilian Labor Force
- Current ratio: 0.13% (13 per 10,000 workers)
- All-time high: 2.997% (April 2020)
- All-time low: 0.122% (October 2022)

**Investment Signal:**
- 4-week moving average rising consistently: Labor market weakening
- Claims >400k for multiple weeks: Recession signal
- Sharp weekly spikes: Market volatility expected

---

### 1.6 Consumer Confidence/Sentiment

| Attribute | Value |
|-----------|-------|
| **FRED Series** | UMCSENT (Michigan), CSCICP03USM665S (Conference Board) |
| **Frequency** | Monthly |
| **Lead Time** | 2-4 months for consumer spending |
| **Historical Accuracy** | Moderate (sentiment vs spending can diverge) |
| **Data Source** | University of Michigan, Conference Board |

**Description:**
Measures household confidence in current and future economic conditions. Consumer spending accounts for ~70% of U.S. GDP.

**University of Michigan Index Components:**
- Index of Current Economic Conditions (CEI)
- Index of Consumer Expectations (ICE)
- Inflation expectations (1-year and 5-year)

**Current Status (January 2026):**
- Index at 56.4 (revised up from 54.0)
- Still >20% below year-ago level
- ~2.4 standard deviations below historical average

**Investment Signal:**
- Expectations component leads spending by several months
- Sentiment diverging from actual spending data: Watch "hard" data (employment, income)
- Sub-50 readings: Historically associated with market caution

**Caveat:**
Recent research shows disconnect between sentiment (negative) and actual spending (positive). Hard data (employment, income) more reliable than sentiment surveys in current environment.

---

### 1.7 Stock Market (S&P 500)

| Attribute | Value |
|-----------|-------|
| **FRED Series** | SP500 |
| **Frequency** | Daily |
| **Lead Time** | 3-9 months |
| **Historical Accuracy** | Moderate (many false signals) |
| **Data Source** | S&P Global |

**Description:**
The stock market itself is a leading indicator, reflecting aggregate expectations of future corporate profits and economic conditions. However, it generates many false signals.

**Famous Quote:**
"The stock market has predicted nine of the last five recessions." - Paul Samuelson

**Investment Signal:**
- 20%+ decline from peak: Bear market, elevated recession risk
- Sustained advance: Economic expansion expected
- Use in combination with other indicators for confirmation

---

## 2. Coincident Indicators

Coincident indicators change direction **at approximately the same time** as the overall economy, confirming current economic conditions.

### 2.1 Nonfarm Payrolls (Employment)

| Attribute | Value |
|-----------|-------|
| **FRED Series** | PAYEMS (total), USPRIV (private) |
| **Frequency** | Monthly (1st Friday) |
| **Lead Time** | Coincident |
| **Market Impact** | Very High |
| **Data Source** | Bureau of Labor Statistics |

**Description:**
Most watched employment indicator. Major market-moving event.

**Recent Data (December 2025):**
- +50,000 jobs (below 60,000 expectation)
- Full 2025: +584,000 (avg 49k/month vs 2.0M in 2024)
- Unemployment rate: 4.4%

**Investment Signal:**
- Surprise vs expectations drives immediate market reaction
- Two-month revisions can materially shift interpretation
- Strong NFP + rising wages: Fed tightening expectations

**Sector Employment Leaders (December 2025):**
- Food services: +27k
- Healthcare: +21k
- Social assistance: +17k
- Retail trade: -25k (weakness)

---

### 2.2 Unemployment Rate

| Attribute | Value |
|-----------|-------|
| **FRED Series** | UNRATE |
| **Frequency** | Monthly |
| **Lead Time** | Coincident to slightly lagging |
| **Historical Accuracy** | High for recession confirmation |
| **Data Source** | Bureau of Labor Statistics |

**Description:**
Percentage of labor force actively seeking employment. Combines with initial claims for comprehensive labor picture.

**Key Levels:**
- <4.0%: Full employment territory
- 4.0-5.0%: Healthy range
- >5.0%: Elevated, potential recession
- Current: 4.4% (August 2025)

**Sahm Rule Connection:**
- 3-month average rising 0.5pp above 12-month low triggers recession signal
- See Section 5.1 for detailed analysis

---

### 2.3 Industrial Production Index

| Attribute | Value |
|-----------|-------|
| **FRED Series** | INDPRO (total), IPMAN (manufacturing) |
| **Frequency** | Monthly |
| **Lead Time** | Coincident |
| **Historical Accuracy** | High |
| **Data Source** | Federal Reserve |

**Description:**
Measures real output in manufacturing, mining, and utilities. Highly cyclical and closely tracks business cycle.

**Key Metrics:**
- **Capacity Utilization** (TCU): Low utilization = weak demand
- **Manufacturing Output** (IPMAN): Core industrial activity
- Index base: 2017 = 100

**Investment Signal:**
- Declining IP + Low capacity utilization: Fiscal/monetary stimulus likely
- Rising IP: Cyclical stocks favored
- Manufacturing weakness spreading to services: Broader slowdown

---

### 2.4 Personal Income and Retail Sales

| Attribute | Value |
|-----------|-------|
| **FRED Series** | PI (personal income), RSAFS (retail sales) |
| **Frequency** | Monthly |
| **Lead Time** | Coincident |
| **Historical Accuracy** | High |
| **Data Source** | Bureau of Economic Analysis, Census Bureau |

**Description:**
Direct measures of consumer financial health and spending activity.

**Personal Income (Q3 2025):**
- Consumer spending: $16,585.90 billion
- Key component: Real disposable personal income (DSPIC96)

**Retail Sales (November 2025):**
- +0.6% month-over-month
- +3.1% year-over-year
- Nonstore retailers: +7.2% YoY
- Food service: +4.9% YoY

**Investment Signal:**
- Income growth outpacing spending: Consumer balance sheets strengthening
- Spending outpacing income: Debt-driven consumption (unsustainable)
- Retail sales weakness: Consumer discretionary sector risk

---

## 3. Lagging Indicators

Lagging indicators change direction **after** the business cycle has already turned, confirming trends in retrospect.

### 3.1 CPI Inflation

| Attribute | Value |
|-----------|-------|
| **FRED Series** | CPIAUCSL (all items), CPILFESL (core) |
| **Frequency** | Monthly |
| **Lead Time** | Lagging (3-12 months) |
| **Historical Accuracy** | High for trend confirmation |
| **Data Source** | Bureau of Labor Statistics |

**Description:**
Measures average change in prices paid by consumers. Classic lagging indicator because prices adjust slowly.

**Why CPI Lags:**
- Long-term contracts (wages, rents, supplies) lock in pricing
- Companies slow to adjust hiring and pricing decisions
- Takes time to recognize permanence of economic shifts

**Investment Signal:**
- Elevated CPI after cycle peak: Stagflation risk
- Declining CPI in expansion: Supports continued Fed accommodation
- Real-time alternatives: Breakeven inflation rates (TIPS), commodity prices

**Sector Implications:**
| Inflation Environment | Favored Sectors |
|----------------------|-----------------|
| Rising inflation | Energy, Real Estate, Commodities |
| Falling inflation | Technology, Growth stocks |
| Stagflation | Consumer Staples, Utilities |

---

### 3.2 Corporate Profits

| Attribute | Value |
|-----------|-------|
| **FRED Series** | CP (corporate profits), A446RC1Q027SBEA (after-tax) |
| **Frequency** | Quarterly |
| **Lead Time** | Lagging |
| **Historical Accuracy** | High |
| **Data Source** | Bureau of Economic Analysis |

**Description:**
Aggregate corporate earnings. Confirms economic strength/weakness but reports with significant delay.

**Investment Signal:**
- Profits declining for 2+ quarters: Recession confirmation
- Profit margins compressing: Earnings revisions likely
- Use forward estimates (analyst consensus) for leading signal

---

### 3.3 Bank Lending and Credit Growth

| Attribute | Value |
|-----------|-------|
| **FRED Series** | TOTLL (total loans), BUSLOANS (commercial) |
| **Frequency** | Weekly/Monthly |
| **Lead Time** | Lagging |
| **Historical Accuracy** | High |
| **Data Source** | Federal Reserve |

**Description:**
Measures credit availability and demand. Banks are slow to adjust lending standards.

**Key Metrics:**
- Commercial & Industrial (C&I) loans
- Consumer credit (credit cards, auto loans)
- Real estate loans
- Bank lending to GDP ratio

**Warning Signal:**
- Rapid increase (5-10 pp in a year) can precede banking crises
- Senior Loan Officer Survey: Forward-looking lending standards

**Current Environment (2025):**
- Banks tightened lending standards (especially commercial)
- Consumer loan growth slowing
- Private credit emerging as alternative financing source

---

### 3.4 Duration of Unemployment

| Attribute | Value |
|-----------|-------|
| **FRED Series** | UEMPMEAN (mean duration), LNS13008397 (median) |
| **Frequency** | Monthly |
| **Lead Time** | Lagging |
| **Historical Accuracy** | High |
| **Data Source** | Bureau of Labor Statistics |

**Description:**
Measures how long unemployed workers have been searching for jobs. Rises well after recession begins and falls well after recovery starts.

**Investment Signal:**
- Rising duration: Labor market scarring, slower recovery
- Falling duration: Recovery strengthening
- Median vs mean: Mean more affected by long-term unemployed

---

## 4. Sector-Specific Indicators

### 4.1 Technology Sector

| Indicator | FRED Series | Frequency | Lead Time |
|-----------|-------------|-----------|-----------|
| Semiconductor Production | PCU33443344 | Monthly | Leading |
| IT Investment | PNFI | Quarterly | Coincident |
| Tech IPO Activity | N/A | Quarterly | Leading |

**Key Semiconductor Indicators:**
- WSTS semiconductor sales (monthly)
- Semiconductor book-to-bill ratio
- Capital expenditure trends (CAPEX)
- R&D investment ratios

**Investment Signal:**
- Semiconductor orders rising: Technology sector expansion
- Book-to-bill >1.0: Demand exceeds supply (bullish)
- Memory chip prices rising: Component shortage (mixed signal)

**Data Sources:**
- SEMI (Semiconductor Equipment and Materials International)
- WSTS (World Semiconductor Trade Statistics)
- SIA (Semiconductor Industry Association)

---

### 4.2 Financial Sector

| Indicator | FRED Series | Frequency | Lead Time |
|-----------|-------------|-----------|-----------|
| Yield Curve | T10Y2Y | Daily | Leading |
| Credit Spreads | BAMLH0A0HYM2 | Daily | Leading |
| Bank Lending | TOTLL | Weekly | Lagging |
| Net Interest Margin | USG10NYR | Daily | Coincident |

**Credit Spreads (Critical Leading Indicator):**
- ICE BofA High Yield Option-Adjusted Spread
- Measures difference between high-yield corporate bonds and Treasuries
- Widening spreads often precede stock market corrections

**Historical Credit Spread Performance:**
| Event | Spread Behavior | Lead Time |
|-------|-----------------|-----------|
| 2007-08 Financial Crisis | Widened mid-2007 | 6+ months before crash |
| 2020 COVID Crash | Spiked early 2020 | Weeks before March crash |
| GFC Peak | 622 bps | N/A |
| COVID Peak | 401 bps | N/A |

**Investment Signal:**
- Spreads near historical lows: No imminent recession signal
- Rapid widening: Risk-off positioning recommended
- Spreads leading stock prices lower: Early warning

---

### 4.3 Energy Sector

| Indicator | Data Source | Frequency | Lead Time |
|-----------|-------------|-----------|-----------|
| EIA Crude Inventory | EIA | Weekly (Wed 10:30 ET) | Coincident |
| API Crude Inventory | API | Weekly (Tue 4:30 ET) | Leading (vs EIA) |
| Baker Hughes Rig Count | Baker Hughes | Weekly | Lagging |
| OPEC Production | OPEC | Monthly | Coincident |

**EIA Petroleum Status Report:**
- Higher than expected inventory: Bearish for crude prices
- Lower than expected inventory: Bullish for crude prices
- Refinery utilization rates: Demand indicator

**Rig Count Dynamics:**
- Rig count fell 13% in 2025 despite record production
- Well productivity improvements offset lower rig count
- Rig count is leading indicator for oil services demand

**Investment Signal:**
- Inventory builds + Falling rig count: Oversupply, bearish energy stocks
- Inventory draws + Rising rig count: Tight market, bullish
- Production efficiency gains: Watch for disconnection from rig count

---

### 4.4 Consumer Sector

| Indicator | FRED Series | Frequency | Lead Time |
|-----------|-------------|-----------|-----------|
| Consumer Confidence | UMCSENT | Monthly | Leading |
| Retail Sales | RSAFS | Monthly | Coincident |
| Credit Card Balances | CCLACBW027NBOG | Monthly | Lagging |
| Auto Sales | TOTALSA | Monthly | Coincident |

**Consumer Health Assessment:**
- Employment + Income + Savings rate + Credit utilization
- Trading down signals: Lower average transaction values
- Card swipes increasing but ticket size decreasing: Budget consciousness

**Investment Signal:**
- Strong employment + Weak sentiment: Opportunity in consumer discretionary
- Weak employment + Strong spending: Unsustainable, caution warranted
- Credit card delinquencies rising: Consumer stress, defensive positioning

---

### 4.5 Housing/Real Estate Sector

| Indicator | FRED Series | Frequency | Lead Time |
|-----------|-------------|-----------|-----------|
| Building Permits | PERMIT | Monthly | Leading |
| Housing Starts | HOUST | Monthly | Leading |
| Existing Home Sales | EXHOSLUSM495S | Monthly | Coincident |
| Case-Shiller Index | CSUSHPISA | Monthly (2-mo lag) | Lagging |

**Housing as Economic Barometer:**
- First to show pain in downturns
- First to recover in expansions
- Housing price growth +4% YoY (2025 Q1) not typical of pre-recession

**Investment Signal:**
- Permits falling + Starts falling: Housing recession, broader slowdown likely
- Mortgage rates falling + Permits rising: Housing recovery
- Multi-family outpacing single-family: Rental demand, affordability constraints

---

## 5. Multi-Indicator Models

### 5.1 Sahm Rule

| Attribute | Value |
|-----------|-------|
| **FRED Series** | SAHMREALTIME, SAHMCURRENT |
| **Trigger** | 3-mo avg unemployment rises 0.5pp above 12-mo low |
| **Historical Accuracy** | 100% (1950-present, one near-miss in 1959) |
| **Lead Time** | ~3 months into recession (not leading) |

**How It Works:**
```
Sahm_Indicator = 3mo_avg(Unemployment) - 12mo_low(3mo_avg(Unemployment))
If Sahm_Indicator >= 0.50 → Recession signal
```

**Performance:**
- Triggered during every recession since 1950
- Average signal: ~3 months after recession start
- Signals before NBER official declaration and before GDP data

**Limitations:**
- 2024 apparent trigger may reflect immigration effects, not demand weakness
- Not a leading indicator (coincident at best)

**Enhanced Versions:**
- **SOS (Scavette-O'Trakoun-Sahm)**: Fewer false positives, earlier signals
- **Michez Rule**: Combines unemployment and vacancy rates, works back to 1930

---

### 5.2 New York Fed Recession Probability Model

| Attribute | Value |
|-----------|-------|
| **FRED Series** | RECPROUSM156N (12-month probability) |
| **Input** | 10Y-3M Treasury spread |
| **Model Type** | Probit regression |
| **Historical Accuracy** | High (predicted 8 of 8 recent recessions) |

**Interpretation:**
- >30% probability: Elevated recession risk
- >50% probability: High recession risk
- Near 0%: Low recession risk

**Current Limitation:**
Extended 2022-2023 inversion generated high probabilities but no recession (yet) materialized.

---

### 5.3 St. Louis Fed Smoothed Recession Probability

| Attribute | Value |
|-----------|-------|
| **FRED Series** | RECPROUSM156N |
| **Inputs** | 4 coincident variables |
| **Model Type** | Dynamic-factor Markov-switching |

**Input Variables:**
1. Nonfarm payroll employment
2. Industrial production index
3. Real personal income (ex-transfers)
4. Real manufacturing and trade sales

**Investment Signal:**
- Probability rising toward 30%: Recession confirmation
- Probability near 0%: Expansion confirmed
- Real-time estimates available

---

### 5.4 Atlanta Fed GDPNow

| Attribute | Value |
|-----------|-------|
| **URL** | atlantafed.org/cqer/research/gdpnow |
| **Frequency** | Multiple updates per week |
| **Lead Time** | Nowcast (current quarter) |
| **Model Type** | Bridge equations + factor models |

**Description:**
Real-time GDP estimate aggregating 13 subcomponents. Updates as new economic data releases.

**Investment Signal:**
- GDPNow tracking negative: Recession risk
- Large revisions from one release: Key data releases identified
- Divergence from consensus: Trading opportunity

---

### 5.5 New York Fed Staff Nowcast

| Attribute | Value |
|-----------|-------|
| **URL** | newyorkfed.org/research/policy/nowcast |
| **Model Type** | Dynamic factor model |
| **Frequency** | Weekly |

**Description:**
Alternative nowcast using large dataset of indicators.

---

### 5.6 J.P. Morgan Recession Probability Framework

**5-Factor Model:**
1. Yield curve
2. Unemployment rate / Sahm Rule
3. Credit spreads
4. Housing market
5. Corporate profit expectations

**Current Assessment (2025):**
- Overall probability: 40% (down from 60%)
- Excluding yield curve: 12%
- Subjective view: 20% (reflecting tariff/trade war risk)

---

## 6. Data Sources

### 6.1 Free Sources

| Source | URL | Key Data | API |
|--------|-----|----------|-----|
| **FRED** | fred.stlouisfed.org | 800,000+ series | Yes (free key) |
| **BLS** | bls.gov/data | Employment, CPI, PPI | Yes |
| **Census Bureau** | census.gov | Retail sales, housing, trade | Yes |
| **BEA** | bea.gov | GDP, personal income/spending | Yes |
| **Federal Reserve** | federalreserve.gov/data | Interest rates, bank data | Yes |
| **EIA** | eia.gov | Energy data | Yes |
| **ISM** | ismworld.org | PMI data | Limited |

### 6.2 FRED API Details

**Registration:**
- Free API key: https://fred.stlouisfed.org/docs/api/api_key.html
- No usage limits for reasonable volume
- Supports XML and JSON responses

**Python Library:**
```python
# Install
pip install fredapi

# Usage
from fredapi import Fred
fred = Fred(api_key='your_key')

# Get series
data = fred.get_series('UNRATE')

# Historical revisions (critical for backtesting)
data = fred.get_series_all_releases('GDP')
```

**Key Features:**
- ALFRED access for point-in-time data (avoiding look-ahead bias)
- Vintage dates for historical data as it was known
- Series search and metadata

### 6.3 Key FRED Series for NDP

| Category | Series ID | Description |
|----------|-----------|-------------|
| **Recession Signals** | | |
| | T10Y2Y | 10-Year minus 2-Year Treasury spread |
| | T10Y3M | 10-Year minus 3-Month Treasury spread |
| | SAHMREALTIME | Sahm Rule indicator |
| | RECPROUSM156N | Recession probability |
| **Employment** | | |
| | PAYEMS | Total nonfarm payrolls |
| | UNRATE | Unemployment rate |
| | ICSA | Initial claims |
| **Production** | | |
| | INDPRO | Industrial production |
| | NAPM | ISM Manufacturing PMI |
| **Consumer** | | |
| | UMCSENT | Consumer sentiment |
| | RSAFS | Retail sales |
| | PCE | Personal consumption expenditure |
| **Housing** | | |
| | PERMIT | Building permits |
| | HOUST | Housing starts |
| **Financial** | | |
| | BAMLH0A0HYM2 | High yield credit spread |
| | TOTLL | Total bank loans |
| | SP500 | S&P 500 index |
| **Inflation** | | |
| | CPIAUCSL | CPI all items |
| | T5YIE | 5-Year breakeven inflation |

---

## 7. Recommended Indicator Set for NDP

### 7.1 Core Recession Monitoring Dashboard

**Tier 1: Primary Signals (High Confidence)**

| Indicator | Weight | Update Frequency | FRED Series |
|-----------|--------|------------------|-------------|
| Yield Curve (10Y-3M) | 25% | Daily | T10Y3M |
| Credit Spreads (HY) | 20% | Daily | BAMLH0A0HYM2 |
| Building Permits | 20% | Monthly | PERMIT |
| Initial Claims | 15% | Weekly | ICSA |
| Sahm Rule | 20% | Monthly | SAHMREALTIME |

**Tier 2: Confirmation Signals**

| Indicator | Purpose | Update Frequency |
|-----------|---------|------------------|
| ISM Manufacturing PMI | Sector health | Monthly |
| Consumer Confidence | Spending outlook | Monthly |
| LEI | Composite view | Monthly |
| Industrial Production | Output confirmation | Monthly |

### 7.2 Sector Rotation Framework

**Expansion Phase (PMI >50, rising)**
- Overweight: Technology, Financials, Industrials, Consumer Discretionary
- Underweight: Utilities, Consumer Staples

**Late Cycle (PMI >50, falling)**
- Overweight: Energy, Materials, Healthcare
- Underweight: Growth stocks, High-multiple tech

**Contraction (PMI <50)**
- Overweight: Utilities, Consumer Staples, Healthcare
- Underweight: Financials, Industrials, Consumer Discretionary

**Recovery (PMI rising toward 50)**
- Overweight: Cyclicals, Small caps, Financials
- Underweight: Defensive sectors

### 7.3 Alert Thresholds

| Indicator | Yellow Alert | Red Alert |
|-----------|--------------|-----------|
| Yield Curve (10Y-3M) | Flat (0-25 bps) | Inverted (<0) |
| Credit Spreads | >400 bps | >600 bps |
| Building Permits | -10% YoY | -20% YoY |
| Initial Claims | >350k 4-wk avg | >400k 4-wk avg |
| Sahm Rule | 0.3-0.49 | >=0.50 |
| ISM PMI | 48-50 | <47 |

### 7.4 Data Refresh Schedule

| Frequency | Indicators | Action |
|-----------|------------|--------|
| **Daily** | Yield curve, Credit spreads | Monitor dashboards |
| **Weekly** | Initial claims, EIA oil data | Update recession probability |
| **Monthly** | Employment, PMI, Housing, CPI | Full indicator review |
| **Quarterly** | GDP, Corporate profits | Strategic assessment |

---

## 8. Implementation Notes

### 8.1 Backtesting Considerations

**Avoiding Look-Ahead Bias:**
- Use ALFRED vintage data for point-in-time accuracy
- Account for publication lags (GDP: ~1 month, employment: ~1 week)
- Use preliminary releases, not revised data

**Data Revisions:**
- Employment data revised for 2 months
- GDP revised for 3 releases (advance, preliminary, final)
- Use fredapi's `get_series_all_releases()` for accuracy

### 8.2 Signal Combination

**Multi-Indicator Approach:**
```
Recession_Score = (
    0.25 * Yield_Curve_Signal +
    0.20 * Credit_Spread_Signal +
    0.20 * Housing_Signal +
    0.15 * Claims_Signal +
    0.20 * Sahm_Signal
)

If Recession_Score > 0.6: High recession probability
If Recession_Score 0.3-0.6: Elevated risk, monitor closely
If Recession_Score < 0.3: Low recession probability
```

**Signal Confirmation:**
- Require 2+ indicators in agreement for high-conviction signals
- Weight more recent data higher for trend detection
- Cross-reference leading and coincident indicators

### 8.3 NDP Integration Architecture

**Bronze Layer (Raw Data):**
- Ingest FRED data via API
- Store with timestamp and vintage date
- Maintain publication lag metadata

**Silver Layer (Processed):**
- Calculate derived indicators (YoY changes, moving averages)
- Compute recession probability scores
- Align data frequencies (daily, weekly, monthly)

**Gold Layer (Features):**
- Regime classification (expansion, late cycle, contraction, recovery)
- Sector rotation signals
- Alert generation

### 8.4 Limitations and Caveats

1. **Model Risk**: Historical relationships may not hold in novel circumstances (e.g., QE effects on yield curve)

2. **False Positives**: All indicators generate false signals; use multiple confirmations

3. **Timing Uncertainty**: Lead times vary significantly across cycles

4. **Data Revisions**: Initial releases often revised substantially

5. **Structural Changes**: Economy evolves; indicator weights may need adjustment

---

## References

### Primary Sources

- [Federal Reserve Economic Data (FRED)](https://fred.stlouisfed.org)
- [Conference Board Leading Economic Index](https://www.conference-board.org/topics/us-leading-indicators/)
- [New York Fed Recession Probability](https://www.newyorkfed.org/research/capital_markets/ycfaq)
- [Atlanta Fed GDPNow](https://www.atlantafed.org/cqer/research/gdpnow)
- [Sahm Rule on FRED](https://fred.stlouisfed.org/series/SAHMREALTIME)

### Research Papers

- Chauvet & Piger: "Real-Time Performance of Business Cycle Dating Methods" (2008)
- Federal Reserve Bank of Chicago: "Why Does the Yield-Curve Slope Predict Recessions?" (2018)
- NBER: "Credit Spreads and Business Cycle Fluctuations" (2011)
- IMF: "GDP Nowcasting Performance of Traditional Econometric Models vs Machine-Learning Algorithms" (2025)

### Additional Resources

- [YCharts Recession Indicators Framework](https://get.ycharts.com/resources/blog/recession-indicators-2025-framework/)
- [J.P. Morgan Recession Probability](https://www.jpmorgan.com/insights/global-research/economy/recession-probability)
- [Morningstar Recession Indicators Cheat Sheet](https://www.morningstar.com/business/insights/blog/markets/leading-recession-indicators)
- [Richmond Fed SOS Indicator](https://www.richmondfed.org/research/national_economy/sos_recession_indicator)

---

## Appendix: Quick Reference Card

### Recession Warning Signs Checklist

- [ ] Yield curve inverted for >3 months
- [ ] Credit spreads widening above 400 bps
- [ ] Building permits down >15% YoY
- [ ] Initial claims 4-wk avg >350k
- [ ] Sahm Rule approaching 0.50
- [ ] ISM PMI below 48
- [ ] Consumer confidence declining sharply
- [ ] Stock market down >15% from peak

### Expansion Confirmation Signs

- [ ] Yield curve positively sloped >100 bps
- [ ] Credit spreads <300 bps
- [ ] Building permits rising YoY
- [ ] Initial claims near historical lows
- [ ] Sahm Rule well below 0.30
- [ ] ISM PMI above 52
- [ ] Consumer confidence rising
- [ ] Employment growth positive

---

*Document Version: 1.0*
*Last Updated: February 2026*
*Author: NDP Research Agent*
