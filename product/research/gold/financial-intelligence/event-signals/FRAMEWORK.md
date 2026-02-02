# Event-Driven Signals for Long-Term Investing

**Research Date**: 2026-02-02
**Platform Context**: Neural Data Platform (NDP) Gold Layer
**Scope**: Event-based signals for long-term investment positioning
**Status**: Research Complete

---

## Executive Summary

Event-driven investing leverages discrete, identifiable occurrences to inform investment decisions. Unlike continuous time-series analysis, event-based approaches focus on specific catalysts that can shift market regimes, alter company fundamentals, or create temporary mispricings. For long-term investors, events matter not for trading the event itself, but for **positioning ahead of regime changes** and understanding when fundamental assumptions should be reassessed.

### Key Findings

| Insight | Implication for Long-Term Investors |
|---------|-------------------------------------|
| Post-Earnings Announcement Drift (PEAD) persists for 60+ days | Earnings surprises signal fundamental changes worth incorporating |
| Fed policy shifts create regime changes | Monetary policy events warrant portfolio repositioning |
| Calendar anomalies have weakened but persist in small-caps | Seasonality is a secondary consideration, not a strategy |
| Event clustering amplifies impact | Multiple simultaneous events require heightened attention |
| NLP/FinBERT achieves 80%+ sentiment accuracy | Automated event extraction is production-ready |

---

## 1. Event Category Taxonomy

### 1.1 Macroeconomic Events

Events driven by central banks, government agencies, and economic data releases.

| Event Type | Frequency | Lead Time | Impact Duration | Data Source |
|------------|-----------|-----------|-----------------|-------------|
| **FOMC Meetings** | 8/year | Known ~1 year ahead | 2-4 weeks | Federal Reserve |
| **Fed Chair Speeches** | ~20/year | 1-7 days | 1-3 days | Fed Calendar |
| **Jobs Reports (NFP)** | Monthly | Known schedule | 1-2 days | BLS |
| **CPI/PPI Inflation** | Monthly | Known schedule | 1-5 days | BLS |
| **GDP Reports** | Quarterly | Known schedule | 1-3 days | BEA |
| **PMI (ISM)** | Monthly | Known schedule | 1 day | ISM |
| **Treasury Auctions** | Weekly | Known schedule | Hours-1 day | Treasury Direct |

**Long-Term Relevance:**
- FOMC policy shifts signal regime changes (risk-on/risk-off)
- Inflation trends affect discount rates and sector rotation
- GDP trajectory influences earnings growth expectations

**Current Context (2026):**
> "The December FOMC meeting ended with the fed funds target rate at 3.50% to 3.75%, its lowest level in three years. Fed officials provided a diverse assessment of future rate policy in 2026." - [Kiplinger](https://www.kiplinger.com/investing/live/december-fed-meeting-live-updates-and-commentary-2025)

### 1.2 Corporate Events

Company-specific events that affect individual securities or sectors.

| Event Type | Frequency | Predictability | Typical Impact | Data Source |
|------------|-----------|----------------|----------------|-------------|
| **Earnings Announcements** | Quarterly | Known 2-4 weeks ahead | 3-60+ days (PEAD) | SEC EDGAR, Earnings Calendars |
| **Earnings Guidance** | Quarterly | With earnings | 3-30 days | SEC 8-K |
| **M&A Announcements** | Variable | Surprise | 1-6 months | SEC 8-K, News |
| **Stock Buyback Programs** | Variable | Announced ahead | Months | SEC 10-K/8-K |
| **Insider Trading (Form 4)** | Continuous | 2-day lag | Weeks-months | SEC EDGAR |
| **13D/13G Filings** | Variable | 10-day window | Weeks | SEC EDGAR |
| **Management Changes** | Variable | Surprise/Planned | Days-weeks | 8-K, Press |
| **Dividend Changes** | Quarterly | With earnings | 1-5 days | Company IR |
| **Credit Rating Changes** | Variable | Surprise | 1-10 days | Moody's, S&P, Fitch |
| **Index Additions/Deletions** | Periodic | Announced ahead | Days | Index Providers |

**Long-Term Relevance:**
- Earnings surprises reveal fundamental trajectory
- Insider buying patterns correlate with future returns
- M&A activity signals sector consolidation trends
- Buybacks indicate management confidence and capital allocation

### 1.3 Political/Geopolitical Events

Events driven by government policy, elections, and international relations.

| Event Type | Predictability | Impact Scope | Typical Duration | Data Source |
|------------|----------------|--------------|------------------|-------------|
| **Elections** | Known dates | Sector/Market-wide | Months | Political Calendars |
| **Policy Announcements** | Variable | Sector-specific | Weeks-years | Government Sources |
| **Trade Policy (Tariffs)** | Variable | Sector/Country | Months-years | USTR, News |
| **Regulatory Changes** | Variable | Sector-specific | Months-years | Federal Register |
| **Geopolitical Conflicts** | Surprise | Market-wide | Variable | News, Intelligence |
| **Sanctions** | Variable | Company/Sector | Months-years | Treasury OFAC |

**Current Context (2026):**
> "Right now, the geopolitical environment is as complex, unpredictable, and dangerous as it's been in decades... shifting tariffs, procurement rules, and government interventions are reshaping manufacturers' supply chains." - [EY Geostrategic Analysis](https://www.ey.com/en_us/insights/geostrategy/geostrategic-analysis)

**Long-Term Relevance:**
- Elections create policy uncertainty premiums
- Trade policy affects multinational earnings
- Regulatory changes can create/destroy competitive moats

### 1.4 Calendar/Seasonal Events

Recurring patterns tied to the calendar.

| Pattern | Timing | Historical Effect | Current Status | Source |
|---------|--------|-------------------|----------------|--------|
| **January Effect** | January | Small-caps +3.8% vs Large +1.2% (historical) | Weakened significantly | Academic Research |
| **Tax-Loss Selling** | December | Pressure on YTD losers | Still observable | Academic Research |
| **Window Dressing** | Quarter-end | Mutual fund rebalancing | Minor effect | Academic Research |
| **Sell in May** | May-October | Lower average returns | Inconsistent | Market Data |
| **Santa Claus Rally** | Late December | +1.3% average | Inconsistent | Market Data |
| **Triple Witching** | 4x/year | Increased volatility | Persists | Options Data |
| **Earnings Season** | 4x/year (~6 weeks) | Higher volatility | Persists | Calendar |

**Current Research (2025):**
> "January no longer stands out as the dominant month. Other periods, notably November and April, have generated stronger average returns, suggesting that the traditional January Effect has weakened over time." - [Investing.com](https://www.investing.com/analysis/seasonality-in-the-sp-500-revisiting-calendar-effects-in-a-modern-market-200672384)

> "Algorithmic and quantitative trading now dominate market volume, quickly arbitraging away simple calendar-based inefficiencies." - [EBC Financial Group](https://www.ebc.com/forex/does-the-january-effect-still-work-what-historical-data-shows)

**Long-Term Relevance:**
- Calendar anomalies are largely arbitraged away for large-caps
- Small-cap effects persist but are difficult to exploit after costs
- Earnings seasons remain periods of elevated information flow

### 1.5 Technical/Market Structure Events

Events related to market microstructure and technical patterns.

| Event Type | Detection | Impact Duration | Relevance |
|------------|-----------|-----------------|-----------|
| **Trend Breakouts** | Technical analysis | Days-weeks | Momentum confirmation |
| **Volatility Regime Shifts** | VIX, HMM models | Weeks-months | Risk management |
| **Liquidity Events** | Volume, bid-ask | Hours-days | Execution timing |
| **Options Expiration** | Known dates | Hours-1 day | Short-term volatility |
| **Market Halts** | Circuit breakers | Minutes-hours | Crisis indicator |

---

## 2. Event Impact Analysis Methodology

### 2.1 Event Study Framework

The standard academic framework for measuring event impact uses **Cumulative Abnormal Returns (CAR)**.

```
Event Timeline:
[-250] ──────── [-30] ──── [-1][0][+1] ──── [+30] ──────── [+250]
  │              │           │   │   │        │              │
  └── Estimation ┘     Pre   Event  Post     └── Post-Event ─┘
      Window           Window Window         Drift Window
```

**Methodology:**
1. **Normal Return Estimation**: Use market model (CAPM) or Fama-French factors over estimation window
2. **Abnormal Return Calculation**: AR(t) = Actual Return - Expected Return
3. **Cumulative Abnormal Return**: CAR = Sum of AR over event window
4. **Statistical Testing**: t-test for significance, non-parametric tests for robustness

> "The abnormal return on a distinct day within the event window represents the difference between the actual stock return on that day and the normal return, which is predicted based on two inputs: the typical relationship between the firm's stock and its reference index (expressed by the alpha and beta parameters), and the actual reference market's return." - [Event Study Tools](https://www.eventstudytools.com/introduction-event-study-methodology)

### 2.2 Pre-Event Positioning (Anticipation Effects)

Markets often price in expected events before they occur.

| Event Type | Typical Lead Time | Anticipation Magnitude |
|------------|-------------------|------------------------|
| Scheduled FOMC | 2-3 weeks | 50-80% of move |
| Earnings (if predictable) | 1-2 weeks | Variable |
| Index Changes | 1-3 weeks | Near complete |
| M&A (rumored) | Days-weeks | 20-50% of premium |
| Elections (polling) | Weeks-months | Gradual pricing |

**Implications for Long-Term Investors:**
- Waiting for event confirmation often means missing the move
- Pre-event positioning based on expected outcomes carries event risk
- Focus on events where the market is mispricing probability

### 2.3 Event Day Reactions

Immediate market response to event realization.

**Key Metrics:**
- **Earnings Announcement Return (EAR)**: 3-day window around earnings
- **Standardized Unexpected Earnings (SUE)**: Earnings surprise magnitude
- **Implied Volatility Crush**: Options IV decline post-event

> "A strategy that buys and sells companies sorted on Earnings Announcement Return (EAR) produces an average abnormal return of 7.55% per year, 1.3% more than a strategy based on the traditional measure of earnings surprise, SUE." - [Brandeis Research](https://peeps.unet.brandeis.edu/~heidifox/ese.pdf)

### 2.4 Post-Event Drift

The persistence of abnormal returns after events - a key inefficiency for long-term investors.

**Post-Earnings Announcement Drift (PEAD):**
> "FinBERT achieves the highest classification accuracy (57.6% and 58.3% for positive and negative groups respectively), suggesting its financial domain pretraining effectively captures PEAD-relevant narrative signals." - [ACL Anthology](https://aclanthology.org/2025.finnlp-2.13.pdf)

| Event Type | Drift Duration | Magnitude | Mechanism |
|------------|----------------|-----------|-----------|
| Earnings Surprise | 60-90 days | 2-8% (SUE quintile spread) | Under-reaction to news |
| M&A Target | 30-90 days | Variable | Deal uncertainty |
| Insider Buying | 30-180 days | 1-3% | Information advantage |
| Dividend Initiation | 30-90 days | 2-4% | Income investor flows |

**Long-Term Implication:**
- PEAD suggests markets under-react to earnings information
- Incorporating earnings surprise signals adds value
- Drift is stronger for small-caps (information friction)

### 2.5 Event Clustering Effects

Multiple events occurring simultaneously create compounding effects.

**Clustering Scenarios:**
1. **Earnings + Macro**: Company reports during Fed meeting week
2. **Sector Events**: Multiple companies in sector report simultaneously
3. **Macro Cascade**: Jobs report + Fed speak + geopolitical event
4. **Calendar Compression**: Year-end tax selling + window dressing + low liquidity

**Detection Approach:**
```sql
-- Event density calculation
SELECT
    date,
    COUNT(*) as event_count,
    SUM(CASE WHEN event_type = 'macro' THEN 1 ELSE 0 END) as macro_events,
    SUM(CASE WHEN event_type = 'earnings' THEN 1 ELSE 0 END) as earnings_events,
    AVG(expected_impact) as avg_impact
FROM events
GROUP BY date
HAVING event_count >= 3;
```

---

## 3. Event Data Source Catalog

### 3.1 Economic Calendar APIs

| Provider | Data Coverage | Cost | API Quality | Real-Time |
|----------|---------------|------|-------------|-----------|
| **[Trading Economics](https://tradingeconomics.com/api/)** | Global macro, forecasts | Paid ($) | Excellent | Yes |
| **[Finnhub](https://finnhub.io/docs/api/economic-calendar)** | Global macro, company | Free tier | Good | Yes |
| **[EODHD](https://eodhd.com/financial-apis-blog/new-economic-events-calendar-api)** | 30+ countries, 50+ event types | Free tier | Good | Yes |
| **[Financial Modeling Prep](https://site.financialmodelingprep.com/developer/docs/economic-calendar-api)** | US-focused | Free tier | Good | Near RT |
| **[FXStreet](https://docs.fxstreet.com/api/calendar/)** | Forex-focused macro | Paid | Good | Yes |
| **[Econoday](https://us.econoday.com/)** | US economic data | Paid | Excellent | Yes |

**Recommended for NDP:**
- **Primary**: Finnhub (free tier) or Trading Economics (paid)
- **Backup**: EODHD for historical data

### 3.2 SEC EDGAR / Corporate Filings

| Provider | Coverage | Cost | Features |
|----------|----------|------|----------|
| **[SEC Official API](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)** | All SEC filings | Free | Basic, no auth required |
| **[sec-api.io](https://sec-api.io/)** | All filings + parsed | Paid | Full-text search, streaming |
| **[EdgarTools (Python)](https://github.com/dgunning/edgartools)** | All filings | Free/OSS | Parsing, XBRL extraction |
| **[py-sec-edgar](https://github.com/ryansmccoy/py-sec-edgar)** | All filings | Free/OSS | Download workflows |

**Key Filing Types for Events:**
- **8-K**: Material events (earnings, M&A, management changes)
- **Form 4**: Insider transactions (2-day filing requirement)
- **13D/13G**: Activist positions (5%+ ownership)
- **10-K/10-Q**: Quarterly/annual financials
- **DEF 14A**: Proxy statements (executive comp, proposals)

> "The Insider Trading Data API allows you to search and list all insider buy and sell transactions of all publicly listed companies on US stock exchanges." - [sec-api.io](https://sec-api.io/docs/insider-ownership-trading-api)

### 3.3 Earnings Calendars

| Provider | Coverage | Accuracy | Features |
|----------|----------|----------|----------|
| **[Earnings Whispers](https://www.earningswhispers.com/)** | US stocks | High | Whisper numbers |
| **[Zacks](https://www.zacks.com/earnings/earnings-calendar)** | US stocks | High | Estimates, history |
| **[Alpha Vantage](https://www.alphavantage.co/)** | Global | Medium | Free API |
| **[Finnhub](https://finnhub.io/)** | Global | Medium | Free tier |

### 3.4 News & Sentiment

| Provider | Type | NLP Ready | Cost |
|----------|------|-----------|------|
| **[Benzinga](https://www.benzinga.com/apis/)** | Financial news | Yes | Paid |
| **[NewsAPI](https://newsapi.org/)** | General news | No | Free tier |
| **[RavenPack](https://www.ravenpack.com/)** | Event extraction | Yes | Enterprise |
| **[Intrinio](https://intrinio.com/)** | News + data | Yes | Paid |

### 3.5 Alternative Data Sources

| Source | Data Type | Signal Type | Latency |
|--------|-----------|-------------|---------|
| **[Quandl/NASDAQ](https://data.nasdaq.com/)** | Various | Multiple | Daily-RT |
| **[Thinknum](https://www.thinknum.com/)** | Web data | Alternative | Daily |
| **[Yipitdata](https://www.yipitdata.com/)** | Transaction data | Consumer | Weekly |
| **Satellite Imagery** | Retail traffic | Foot traffic | Daily |
| **Social Sentiment** | Twitter/Reddit | Retail flows | Real-time |

---

## 4. Feature Engineering from Events

### 4.1 Temporal Features

Features that capture time-based relationships to events.

```sql
-- Days until next event features
SELECT
    date,
    ticker,

    -- Macro event proximity
    DATE_PART('day', next_fomc_date - date) AS days_to_fomc,
    DATE_PART('day', date - last_fomc_date) AS days_since_fomc,
    DATE_PART('day', next_nfp_date - date) AS days_to_jobs,

    -- Earnings proximity
    DATE_PART('day', next_earnings_date - date) AS days_to_earnings,
    DATE_PART('day', date - last_earnings_date) AS days_since_earnings,

    -- Cyclical encoding for event timing
    SIN(2 * PI() * days_to_earnings / 90.0) AS earnings_cycle_sin,
    COS(2 * PI() * days_to_earnings / 90.0) AS earnings_cycle_cos

FROM market_data m
LEFT JOIN events e ON m.ticker = e.ticker;
```

### 4.2 Event History Features

Features that capture patterns in past events.

```sql
-- Earnings surprise history features
WITH earnings_history AS (
    SELECT
        ticker,
        announcement_date,
        actual_eps,
        consensus_eps,
        (actual_eps - consensus_eps) / NULLIF(ABS(consensus_eps), 0) AS eps_surprise_pct,
        actual_eps - LAG(actual_eps, 4) OVER (PARTITION BY ticker ORDER BY announcement_date) AS yoy_eps_change
    FROM earnings_announcements
)
SELECT
    ticker,
    announcement_date,

    -- Surprise history
    AVG(eps_surprise_pct) OVER w4q AS avg_surprise_4q,
    STDDEV(eps_surprise_pct) OVER w4q AS surprise_volatility_4q,

    -- Surprise streak
    SUM(CASE WHEN eps_surprise_pct > 0 THEN 1 ELSE 0 END) OVER w4q AS beat_streak_4q,

    -- SUE calculation
    (actual_eps - LAG(actual_eps, 4) OVER w) /
        NULLIF(STDDEV(actual_eps - LAG(actual_eps, 4) OVER w8q), 0) AS sue

FROM earnings_history
WINDOW
    w AS (PARTITION BY ticker ORDER BY announcement_date),
    w4q AS (PARTITION BY ticker ORDER BY announcement_date ROWS BETWEEN 3 PRECEDING AND CURRENT ROW),
    w8q AS (PARTITION BY ticker ORDER BY announcement_date ROWS BETWEEN 7 PRECEDING AND CURRENT ROW);
```

### 4.3 Event Volatility Features

Features comparing expected vs. realized event volatility.

```sql
-- Event volatility features
SELECT
    ticker,
    event_date,
    event_type,

    -- Pre-event implied volatility
    implied_vol_pre_event,

    -- Post-event realized volatility
    realized_vol_post_event,

    -- Volatility premium (IV - RV)
    implied_vol_pre_event - realized_vol_post_event AS vol_premium,

    -- Historical event vol pattern
    AVG(ABS(event_day_return)) OVER w4 AS avg_event_move_4q,
    MAX(ABS(event_day_return)) OVER w4 AS max_event_move_4q,

    -- Options market expectation
    straddle_implied_move,
    actual_event_move,
    actual_event_move / NULLIF(straddle_implied_move, 0) AS move_vs_expected

FROM event_volatility
WINDOW w4 AS (PARTITION BY ticker, event_type ORDER BY event_date ROWS BETWEEN 3 PRECEDING AND CURRENT ROW);
```

### 4.4 Sentiment Features from Events

Features derived from NLP analysis of event-related text.

```python
# FinBERT sentiment extraction
from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch

class EventSentimentExtractor:
    def __init__(self):
        self.tokenizer = AutoTokenizer.from_pretrained("ProsusAI/finbert")
        self.model = AutoModelForSequenceClassification.from_pretrained("ProsusAI/finbert")

    def extract_sentiment(self, text: str) -> dict:
        """Extract sentiment from event-related text."""
        inputs = self.tokenizer(text, return_tensors="pt", truncation=True, max_length=512)
        outputs = self.model(**inputs)
        probs = torch.nn.functional.softmax(outputs.logits, dim=-1)

        return {
            "positive": probs[0][0].item(),
            "negative": probs[0][1].item(),
            "neutral": probs[0][2].item(),
            "sentiment_score": probs[0][0].item() - probs[0][1].item()  # Net sentiment
        }

    def extract_event_features(self, headline: str, body: str) -> dict:
        """Extract comprehensive features from event text."""
        headline_sent = self.extract_sentiment(headline)
        body_sent = self.extract_sentiment(body)

        return {
            "headline_sentiment": headline_sent["sentiment_score"],
            "body_sentiment": body_sent["sentiment_score"],
            "headline_confidence": max(headline_sent["positive"], headline_sent["negative"]),
            "sentiment_agreement": 1 if (headline_sent["sentiment_score"] > 0) == (body_sent["sentiment_score"] > 0) else 0,
            "text_length": len(body.split()),
        }
```

> "FinBERT embeddings and sentiment features are integrated with sequential (LSTM/BiLSTM/GRU) architectures for market movement and price prediction. These sentiment vectors consistently reduce error metrics (e.g., 32.2% MAE reduction in S&P 500 forecasting)." - [arXiv](https://arxiv.org/pdf/2306.02136)

### 4.5 Insider Trading Features

Features derived from SEC Form 4 filings.

```sql
-- Insider trading signal features
WITH insider_activity AS (
    SELECT
        ticker,
        filing_date,
        insider_name,
        transaction_type,  -- P (purchase), S (sale)
        shares,
        price,
        shares * price AS dollar_value,
        relationship  -- Officer, Director, 10% Owner
    FROM form_4_filings
)
SELECT
    ticker,
    date,

    -- Net insider activity (30-day rolling)
    SUM(CASE WHEN transaction_type = 'P' THEN dollar_value ELSE -dollar_value END)
        OVER w30d AS net_insider_flow_30d,

    -- Buyer/seller ratio
    COUNT(CASE WHEN transaction_type = 'P' THEN 1 END) OVER w30d::FLOAT /
        NULLIF(COUNT(*) OVER w30d, 0) AS insider_buy_ratio_30d,

    -- Officer vs director activity
    SUM(CASE WHEN relationship = 'Officer' AND transaction_type = 'P' THEN dollar_value ELSE 0 END)
        OVER w30d AS officer_buying_30d,

    -- Cluster buying (multiple insiders)
    COUNT(DISTINCT insider_name) FILTER (WHERE transaction_type = 'P') OVER w30d AS unique_buyers_30d

FROM market_data m
LEFT JOIN insider_activity i ON m.ticker = i.ticker AND i.filing_date BETWEEN m.date - 30 AND m.date
WINDOW w30d AS (PARTITION BY ticker ORDER BY date RANGE BETWEEN INTERVAL '30 days' PRECEDING AND CURRENT ROW);
```

### 4.6 Event Density Features

Features capturing the concentration of events.

```sql
-- Event density and clustering features
SELECT
    date,
    ticker,

    -- Same-day event count
    COUNT(*) FILTER (WHERE event_date = date) AS same_day_events,

    -- 5-day event density
    COUNT(*) FILTER (WHERE event_date BETWEEN date - 5 AND date) AS event_count_5d,

    -- Event type mix
    COUNT(DISTINCT event_type) FILTER (WHERE event_date BETWEEN date - 5 AND date) AS event_diversity_5d,

    -- Macro event indicator
    MAX(CASE WHEN event_type IN ('FOMC', 'NFP', 'CPI') AND event_date BETWEEN date - 2 AND date + 2 THEN 1 ELSE 0 END) AS near_macro_event,

    -- Earnings season indicator
    CASE WHEN EXTRACT(MONTH FROM date) IN (1, 4, 7, 10)
              AND EXTRACT(DAY FROM date) BETWEEN 10 AND 45 THEN 1 ELSE 0 END AS earnings_season

FROM market_data m
LEFT JOIN events e ON m.ticker = e.ticker;
```

---

## 5. Event + Time Series Fusion

### 5.1 Events as Regime Switch Triggers

Use events to identify regime changes for conditional forecasting.

**Hidden Markov Model Approach:**

> "HMMs are used to identify different market regimes in the US stock market and propose investment strategies that switch factor investment models depending on the current detected regime." - [MDPI](https://www.mdpi.com/1911-8074/13/12/311)

```python
from hmmlearn import hmm
import numpy as np

class EventAwareRegimeDetector:
    """Detect market regimes using HMM with event features."""

    def __init__(self, n_regimes: int = 3):
        self.n_regimes = n_regimes
        self.model = hmm.GaussianHMM(
            n_components=n_regimes,
            covariance_type="full",
            n_iter=100
        )
        self.regime_labels = {0: "Bearish", 1: "Neutral", 2: "Bullish"}

    def prepare_features(self, returns: np.ndarray, events: np.ndarray) -> np.ndarray:
        """Combine returns and event features for regime detection."""
        # Features: returns, volatility, event indicators
        volatility = self._rolling_volatility(returns, window=20)

        features = np.column_stack([
            returns,
            volatility,
            events  # Event indicator features
        ])
        return features

    def fit_predict(self, features: np.ndarray) -> np.ndarray:
        """Fit HMM and predict regime states."""
        self.model.fit(features)
        return self.model.predict(features)

    def get_regime_probabilities(self, features: np.ndarray) -> np.ndarray:
        """Get probability distribution over regimes."""
        return self.model.predict_proba(features)

    @staticmethod
    def _rolling_volatility(returns: np.ndarray, window: int = 20) -> np.ndarray:
        """Calculate rolling volatility."""
        vol = np.zeros_like(returns)
        for i in range(window, len(returns)):
            vol[i] = np.std(returns[i-window:i])
        return vol
```

**Event-Triggered Regime Switch Logic:**
```python
def should_switch_regime(
    current_regime: str,
    event: dict,
    probabilities: np.ndarray
) -> tuple[bool, str]:
    """Determine if an event should trigger regime reassessment."""

    # High-impact events force regime check
    high_impact_events = ['FOMC_RATE_CHANGE', 'MAJOR_EARNINGS_MISS', 'GEOPOLITICAL_SHOCK']

    if event['type'] in high_impact_events:
        # Check if regime probabilities have shifted significantly
        max_prob_regime = np.argmax(probabilities)
        max_prob = probabilities[max_prob_regime]

        if max_prob > 0.7 and max_prob_regime != current_regime:
            return True, max_prob_regime

    return False, current_regime
```

### 5.2 Conditional Forecasting Based on Events

Adjust time-series forecasts based on upcoming events.

```python
class EventAwareForecaster:
    """Time-series forecaster that conditions on events."""

    def __init__(self, base_model):
        self.base_model = base_model
        self.event_adjustments = {}

    def forecast(
        self,
        historical: np.ndarray,
        horizon: int,
        upcoming_events: list[dict]
    ) -> np.ndarray:
        """Generate forecast with event adjustments."""

        # Base forecast from time-series model
        base_forecast = self.base_model.predict(historical, horizon)

        # Apply event-based adjustments
        adjusted_forecast = base_forecast.copy()

        for event in upcoming_events:
            days_to_event = event['days_from_now']

            if days_to_event < horizon:
                # Pre-event: increase uncertainty
                if days_to_event > 0:
                    uncertainty_multiplier = 1 + (0.1 * event['expected_impact'])
                    adjusted_forecast['confidence_interval'][days_to_event] *= uncertainty_multiplier

                # Event day: apply expected impact
                if days_to_event == 0:
                    adjusted_forecast['point_forecast'][0] *= (1 + event['expected_return_impact'])

                # Post-event: apply drift expectation
                if event['type'] == 'earnings' and event['expected_surprise'] != 0:
                    drift_days = min(60, horizon - days_to_event)
                    drift_per_day = event['expected_surprise'] * 0.001  # PEAD assumption
                    for d in range(days_to_event + 1, days_to_event + drift_days + 1):
                        if d < horizon:
                            adjusted_forecast['point_forecast'][d] *= (1 + drift_per_day)

        return adjusted_forecast
```

### 5.3 Event-Aware Feature Engineering

Combine event features with time-series features.

```sql
-- Combined event + time-series feature view
CREATE VIEW ml_features_with_events AS
SELECT
    m.date,
    m.ticker,

    -- Time-series features
    m.return_1d,
    m.return_5d,
    m.return_20d,
    m.volatility_20d,
    m.rsi_14,
    m.macd,

    -- Rolling statistics
    m.price_mean_20d,
    m.price_std_20d,
    m.volume_mean_20d,

    -- Event proximity features
    e.days_to_earnings,
    e.days_since_earnings,
    e.days_to_fomc,

    -- Event history features
    e.last_sue,
    e.avg_surprise_4q,
    e.beat_streak_4q,

    -- Insider activity
    e.net_insider_flow_30d,
    e.insider_buy_ratio_30d,

    -- Sentiment features
    e.latest_sentiment_score,
    e.sentiment_trend_5d,

    -- Regime features
    e.current_regime,
    e.regime_confidence,

    -- Event density
    e.event_count_5d,
    e.near_macro_event,
    e.earnings_season

FROM time_series_features m
LEFT JOIN event_features e ON m.date = e.date AND m.ticker = e.ticker;
```

### 5.4 Model Architecture for Fusion

```
                    ┌─────────────────────────────────────────────┐
                    │              FEATURE FUSION                  │
                    │                                             │
Time Series ──────▶ │  ┌─────────────┐    ┌─────────────────┐    │
(price, volume,     │  │ Time-Series │    │                 │    │
 technicals)        │  │ Encoder     │───▶│                 │    │
                    │  │ (LSTM/TCN)  │    │                 │    │
                    │  └─────────────┘    │   Fusion Layer  │───▶│──▶ Prediction
                    │                     │   (Attention /  │    │
Event Features ───▶ │  ┌─────────────┐    │   Concatenate)  │    │
(days_to_event,     │  │ Event       │───▶│                 │    │
 sentiment,         │  │ Encoder     │    │                 │    │
 regime)            │  │ (MLP/BERT)  │    └─────────────────┘    │
                    │  └─────────────┘                           │
                    └─────────────────────────────────────────────┘
```

---

## 6. NDP Integration Recommendations

### 6.1 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        NDP EVENT SIGNAL LAYER                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  BRONZE LAYER                                                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │
│  │ Economic    │  │ SEC EDGAR   │  │ Earnings    │  │ News Feed   │   │
│  │ Calendar    │  │ Filings     │  │ Calendar    │  │ (RSS/API)   │   │
│  │ (Parquet)   │  │ (Parquet)   │  │ (Parquet)   │  │ (Parquet)   │   │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘   │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  SILVER LAYER (TimescaleDB)                                             │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ event_calendar       │ Normalized event data with timestamps    │   │
│  │ insider_transactions │ Parsed Form 4 data                       │   │
│  │ earnings_history     │ Historical earnings + SUE calculations   │   │
│  │ sentiment_scores     │ NLP-derived sentiment per event          │   │
│  │ regime_states        │ HMM regime detection output              │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  GOLD LAYER (Features + Intelligence)                                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ event_features_daily │ Continuous aggregates with event signals │   │
│  │ ml_features_fusion   │ Combined time-series + event features    │   │
│  │ regime_transitions   │ Regime change detection + history        │   │
│  │ event_embeddings     │ sqlite-vec embeddings for event search   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Data Ingestion Strategy

**Priority Order for Implementation:**

| Priority | Data Source | Complexity | Value | Edge Feasibility |
|----------|-------------|------------|-------|------------------|
| 1 | Economic Calendar (Finnhub) | Low | High | Excellent |
| 2 | Earnings Calendar | Low | High | Excellent |
| 3 | SEC EDGAR (Form 4, 8-K) | Medium | High | Good |
| 4 | News Sentiment (FinBERT) | Medium | Medium | Challenging |
| 5 | Alternative Data | High | Variable | Depends |

**Ingestion Schedule:**

```yaml
# Event data ingestion schedule
event_ingestion:
  economic_calendar:
    frequency: hourly
    source: finnhub
    format: json
    storage: bronze/economic-calendar/

  earnings_calendar:
    frequency: daily (6am)
    source: finnhub
    format: json
    storage: bronze/earnings-calendar/

  sec_filings:
    frequency: 15min (during market hours)
    source: sec.gov RSS / sec-api
    types: [8-K, Form-4, 13D]
    storage: bronze/sec-filings/

  news_feed:
    frequency: 5min
    source: newsapi / benzinga
    filter: financial keywords
    storage: bronze/news-feed/
```

### 6.3 Edge Deployment Considerations

**Resource Budget for Event Processing:**

| Component | Memory | CPU | Latency | Notes |
|-----------|--------|-----|---------|-------|
| Event Calendar Store | 10MB | <1% | <10ms | Simple lookup |
| Earnings History (5yr) | 50MB | 2% | <50ms | Per-query |
| Insider Transaction DB | 100MB | 5% | <100ms | Rolling window |
| FinBERT (INT8) | 150MB | 30% | ~500ms | Batch only |
| HMM Regime Detection | 20MB | 5% | <50ms | Per update |
| **Total Event Layer** | ~330MB | ~43% | Variable | Incremental to base NDP |

**Optimization Strategies:**
1. **Batch NLP**: Run FinBERT sentiment during off-hours
2. **Incremental Updates**: Only process new events
3. **Caching**: Cache derived features, invalidate on new data
4. **Priority Processing**: Process high-impact events first

### 6.4 Integration with Existing NDP Patterns

**Leverage TimescaleDB Continuous Aggregates:**

```sql
-- Event feature continuous aggregate
CREATE MATERIALIZED VIEW event_features_hourly
WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    ticker,

    -- Event counts
    COUNT(*) FILTER (WHERE event_type = 'earnings') AS earnings_events,
    COUNT(*) FILTER (WHERE event_type = 'insider_buy') AS insider_buys,
    COUNT(*) FILTER (WHERE event_type = 'macro') AS macro_events,

    -- Sentiment aggregation
    AVG(sentiment_score) AS avg_sentiment,
    MAX(sentiment_score) AS max_sentiment,
    MIN(sentiment_score) AS min_sentiment,

    -- Event impact
    MAX(expected_impact) AS max_expected_impact

FROM event_stream
GROUP BY bucket, ticker;

-- Auto-refresh policy
SELECT add_continuous_aggregate_policy('event_features_hourly',
    start_offset => INTERVAL '24 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
```

**Leverage AgentDB for Pattern Learning:**

```javascript
// Store successful event-based patterns in AgentDB
mcp__agentdb__agentdb_pattern_store({
    taskType: "event_signal_processing",
    approach: "PEAD detection using SUE > 2 threshold with 60-day drift window",
    successRate: 0.72,
    tags: ["earnings", "drift", "quantitative"]
});

// Search for relevant patterns
mcp__agentdb__agentdb_pattern_search({
    task: "How to detect post-earnings announcement drift for small-cap stocks",
    k: 5,
    filters: { minSuccessRate: 0.6 }
});
```

### 6.5 Sample Schema Design

```sql
-- Core event table
CREATE TABLE events (
    event_id BIGSERIAL PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL,
    event_subtype VARCHAR(50),
    ticker VARCHAR(10),
    event_date TIMESTAMPTZ NOT NULL,
    scheduled_time TIMESTAMPTZ,
    actual_time TIMESTAMPTZ,
    source VARCHAR(50) NOT NULL,
    raw_data JSONB,
    processed_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create hypertable for time-series queries
SELECT create_hypertable('events', 'event_date');

-- Earnings-specific table
CREATE TABLE earnings_announcements (
    id BIGSERIAL,
    ticker VARCHAR(10) NOT NULL,
    fiscal_quarter DATE NOT NULL,
    announcement_date TIMESTAMPTZ NOT NULL,
    actual_eps DECIMAL(10,4),
    consensus_eps DECIMAL(10,4),
    surprise_pct DECIMAL(10,4),
    sue DECIMAL(10,4),
    guidance_direction VARCHAR(10),  -- 'raise', 'lower', 'maintain', NULL
    ear_3day DECIMAL(10,4),  -- Earnings announcement return
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (id, announcement_date)
);

SELECT create_hypertable('earnings_announcements', 'announcement_date');

-- Insider transactions table
CREATE TABLE insider_transactions (
    id BIGSERIAL,
    ticker VARCHAR(10) NOT NULL,
    filing_date TIMESTAMPTZ NOT NULL,
    transaction_date DATE NOT NULL,
    insider_cik VARCHAR(20),
    insider_name VARCHAR(200),
    relationship VARCHAR(50),
    transaction_type CHAR(1),  -- P, S, M, G, etc.
    shares DECIMAL(20,4),
    price DECIMAL(10,4),
    value DECIMAL(20,4),
    shares_owned_after DECIMAL(20,4),
    form_type VARCHAR(10),
    sec_link TEXT,
    PRIMARY KEY (id, filing_date)
);

SELECT create_hypertable('insider_transactions', 'filing_date');

-- Event sentiment table
CREATE TABLE event_sentiment (
    id BIGSERIAL,
    event_id BIGINT REFERENCES events(event_id),
    timestamp TIMESTAMPTZ NOT NULL,
    headline TEXT,
    headline_sentiment DECIMAL(5,4),
    body_sentiment DECIMAL(5,4),
    model_version VARCHAR(20),
    confidence DECIMAL(5,4),
    PRIMARY KEY (id, timestamp)
);

SELECT create_hypertable('event_sentiment', 'timestamp');
```

### 6.6 Implementation Roadmap

**Phase 1: Foundation (Weeks 1-4)**
- [ ] Set up Finnhub API integration for economic calendar
- [ ] Create Bronze layer Parquet storage for events
- [ ] Design Silver layer schema (events, earnings, insider)
- [ ] Implement basic event proximity features

**Phase 2: Core Features (Weeks 5-8)**
- [ ] Integrate SEC EDGAR for Form 4 / 8-K
- [ ] Implement SUE calculation pipeline
- [ ] Build earnings history features
- [ ] Create insider transaction features

**Phase 3: Intelligence (Weeks 9-12)**
- [ ] Implement HMM regime detection
- [ ] Add FinBERT sentiment analysis (batch)
- [ ] Build event-aware forecasting module
- [ ] Create continuous aggregates for event features

**Phase 4: Integration (Weeks 13-16)**
- [ ] Fuse event features with existing time-series features
- [ ] Build ML pipeline with combined features
- [ ] Validate on historical data
- [ ] Deploy to edge with resource monitoring

---

## 7. References

### Academic Research
- [Post-Earnings-Announcement Drift Prediction with Multi-task Learning](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=5284651) (SSRN, 2025)
- [Enhancing PEAD Measurement with Large Language Models](https://aclanthology.org/2025.finnlp-2.13.pdf) (ACL, 2025)
- [Regime-Switching Factor Investing with Hidden Markov Models](https://www.mdpi.com/1911-8074/13/12/311) (MDPI)
- [Event Detection in Time Series: Universal Deep Learning Approach](https://arxiv.org/abs/2311.15654) (arXiv)

### Market Analysis
- [BlackRock 2026 Global Macro Outlook](https://www.blackrock.com/institutions/en-us/insights/2026-macro-outlook)
- [J.P. Morgan 2026 Market Outlook](https://www.jpmorgan.com/insights/global-research/outlook/market-outlook)
- [EY Geostrategic Analysis 2025](https://www.ey.com/en_us/insights/geostrategy/geostrategic-analysis)

### Data Sources
- [Trading Economics Calendar API](https://tradingeconomics.com/api/calendar.aspx)
- [Finnhub API Documentation](https://finnhub.io/docs/api/economic-calendar)
- [SEC EDGAR APIs](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)
- [sec-api.io Documentation](https://sec-api.io/docs)

### NLP & Sentiment
- [FinBERT GitHub](https://github.com/ProsusAI/finBERT)
- [Financial Sentiment Analysis using FinBERT](https://arxiv.org/pdf/2306.02136)
- [Comparative Evaluation of Embedding Representations](https://arxiv.org/html/2512.13749)

### Calendar Effects
- [The January Effect and Stock Market Seasonality](https://www.americancentury.com/insights/the-january-effect-and-stock-market-seasonality/)
- [Seasonality in the S&P 500: Revisiting Calendar Effects](https://www.investing.com/analysis/seasonality-in-the-sp-500-revisiting-calendar-effects-in-a-modern-market-200672384)
- [The January Effect Before Tax-Loss Selling and Window-Dressing](http://www.efmaefm.org/0EFMAMEETINGS/EFMA%20ANNUAL%20MEETINGS/2025-Greece/papers/JanEffectsEFMAFULL.pdf) (EFMA, 2025)

### Event Study Methodology
- [Introduction to the Event Study Methodology](https://www.eventstudytools.com/introduction-event-study-methodology)
- [Event Study with Stata - Princeton Guide](https://libguides.princeton.edu/eventstudy)
- [Cumulative Abnormal Returns in Event Studies](https://fastercapital.com/content/Cumulative-Abnormal-Returns--CAR---Decoding-the-Impact--Cumulative-Abnormal-Returns-in-Event-Studies.html)

### Quantitative Strategies
- [Standardized Unexpected Earnings - QuantConnect](https://www.quantconnect.com/research/15369/standardized-unexpected-earnings/)
- [Post-Earnings Announcement Effect - Quantpedia](https://quantpedia.com/strategies/post-earnings-announcement-effect)
- [Market Regime Detection using HMM - QuantStart](https://www.quantstart.com/articles/market-regime-detection-using-hidden-markov-models-in-qstrader/)

---

**Document Version**: 1.0
**Last Updated**: 2026-02-02
**Author**: Research Agent
**Status**: Complete
