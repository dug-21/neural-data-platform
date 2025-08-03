# Sector ETF Reference Guide

## Overview

This document provides a comprehensive reference for the ETF symbols used to represent each market sector in the Neural Trader system. These ETFs serve as benchmarks for sector performance validation and correlation analysis.

## Primary Sector ETF Mappings

| Sector | ETF Symbol | ETF Name | Description | Key Holdings |
|--------|------------|----------|-------------|--------------|
| **Technology** | XLK | Technology Select Sector SPDR Fund | Large-cap U.S. technology companies | AAPL, MSFT, NVDA |
| **Financial Services** | XLF | Financial Select Sector SPDR Fund | Banks, insurance, financial services | JPM, BRK.B, BAC |
| **Healthcare** | XLV | Health Care Select Sector SPDR Fund | Pharmaceuticals, biotech, healthcare | JNJ, UNH, PFE |
| **Energy** | XLE | Energy Select Sector SPDR Fund | Oil, gas, energy equipment | XOM, CVX, COP |
| **Consumer Discretionary** | XLY | Consumer Discretionary Select SPDR | Retail, restaurants, leisure | AMZN, TSLA, HD |
| **Consumer Staples** | XLP | Consumer Staples Select Sector SPDR | Food, beverages, household products | PG, KO, WMT |
| **Industrials** | XLI | Industrial Select Sector SPDR Fund | Manufacturing, transportation | BA, CAT, UNP |
| **Materials** | XLB | Materials Select Sector SPDR Fund | Chemicals, mining, construction | DOW, DD, NEM |
| **Utilities** | XLU | Utilities Select Sector SPDR Fund | Electric, gas, water utilities | NEE, DUK, SO |
| **Real Estate** | XLRE | Real Estate Select Sector SPDR Fund | REITs, real estate companies | AMT, PLD, CCI |

## Alternative Sector ETFs

For broader coverage or different perspectives, these alternative ETFs can be used:

### Technology Alternatives
- **QQQ** - Invesco QQQ Trust (NASDAQ-100 focused)
- **VGT** - Vanguard Information Technology ETF
- **IYW** - iShares U.S. Technology ETF

### Financial Alternatives
- **KBE** - SPDR S&P Bank ETF (banking focused)
- **KIE** - SPDR S&P Insurance ETF (insurance focused)
- **IYF** - iShares U.S. Financials ETF

### Healthcare Alternatives
- **IBB** - iShares Biotechnology ETF (biotech focused)
- **IHI** - iShares U.S. Medical Devices ETF
- **VHT** - Vanguard Health Care ETF

## Configuration Location

The ETF mappings are configured in `config/sector_models.toml`:

```toml
[sectors.technology]
etf_representative = "XLK"
sector_name = "Technology"
# ... other settings

[sectors.financial_services]
etf_representative = "XLF"
sector_name = "Financial Services"
# ... other settings
```

## Usage in Neural Trader

### 1. **Sector Validation**
The system uses these ETFs to validate sector aggregations:
- Correlation analysis between sector metrics and ETF performance
- Validation threshold: >0.8 correlation expected

### 2. **Sector Breadth Analysis**
ETF volume and price movements indicate sector-wide trends:
- Rising ETF with declining individual stocks = sector rotation
- Falling ETF with rising individual stocks = stock-specific events

### 3. **Model Training**
Sector models use ETF data as additional features:
- ETF momentum as sector sentiment indicator
- ETF volume spikes as sector interest gauge
- ETF volatility as sector risk measure

### 4. **Risk Management**
ETF correlations help with portfolio risk:
- High correlation between ETFs indicates market-wide movement
- Low correlation suggests sector-specific opportunities

## Data Requirements

For each sector ETF, the system requires:

| Data Type | Update Frequency | Used For |
|-----------|------------------|----------|
| Price | 1-minute | Real-time sector tracking |
| Volume | 1-minute | Sector interest measurement |
| Bid/Ask | 1-minute | Liquidity assessment |
| Options Flow | 5-minute | Sector sentiment (optional) |

## Updating ETF Mappings

To change or add ETF mappings:

1. Edit `config/sector_models.toml`
2. Update the `etf_representative` field for the sector
3. Restart the neural-trader system
4. The system will automatically start using the new ETF

Example:
```toml
[sectors.technology]
etf_representative = "QQQ"  # Changed from XLK to QQQ
```

## ETF Data Quality Monitoring

The system monitors ETF data quality:

- **Stale Data Alert**: If ETF data is >5 minutes old
- **Low Volume Alert**: If ETF volume drops below daily average by 50%
- **Spread Alert**: If bid-ask spread exceeds 0.1% of price
- **Correlation Alert**: If ETF-sector correlation drops below 0.7

## Best Practices

1. **Use SPDR Sector ETFs (XL*)** as primary representatives
   - Most liquid and widely tracked
   - Best correlation with S&P 500 sectors
   - Long history for backtesting

2. **Monitor ETF Changes**
   - ETFs can change holdings quarterly
   - Major rebalancing can affect correlations
   - Keep sector symbol lists updated

3. **Consider Market Hours**
   - ETF data most reliable during market hours
   - Pre/post-market ETF data may be thin
   - Use previous close for off-hours calculations

4. **Validate Regularly**
   - Monthly correlation checks recommended
   - Alert on correlation drops below 0.7
   - Consider alternative ETFs if correlation degrades

## Troubleshooting

### Common Issues:

**ETF data not updating**
- Check Redis channel subscription for ETF symbols
- Verify data provider includes ETF symbols
- Check `etf_representative` spelling in config

**Low correlation with sector**
- Verify sector symbol assignments are correct
- Check for recent sector rotation or rebalancing
- Consider using alternative sector ETF

**Missing ETF data**
- Ensure ETF symbols are in data subscription
- Check for ETF symbol changes or delistings
- Add ETF symbols to symbol universe

## Historical Performance

Typical ETF-Sector correlations (based on historical data):

| Sector | Average Correlation | Volatility |
|--------|-------------------|------------|
| Technology | 0.92 | High |
| Financials | 0.89 | Medium |
| Healthcare | 0.87 | Medium |
| Energy | 0.85 | High |
| Consumer Disc. | 0.88 | Medium |
| Consumer Staples | 0.86 | Low |
| Industrials | 0.87 | Medium |
| Materials | 0.84 | Medium |
| Utilities | 0.83 | Low |
| Real Estate | 0.85 | Medium |

## Future Enhancements

Planned improvements for ETF integration:

1. **Multi-ETF Validation**: Use multiple ETFs per sector for robustness
2. **International ETFs**: Add global sector ETFs for broader perspective
3. **Factor ETFs**: Include growth/value sector ETFs for style analysis
4. **Custom Indices**: Build proprietary sector indices from constituents

## See Also

- **[NEURAL_TRADER_CONFIGURATION_GUIDE.md](NEURAL_TRADER_CONFIGURATION_GUIDE.md)**: Complete configuration guide for all settings
- **[config/sector_models.toml](/Users/dmf/repos/neural-trader/config/sector_models.toml)**: Actual configuration file with ETF mappings
- **[HIGH_LEVEL_FEATURE_PLAN.md](HIGH_LEVEL_FEATURE_PLAN.md)**: Overall transformation plan

---

*Last Updated: Phase 2 Implementation*
*Configuration File: `config/sector_models.toml`*
*Configuration Guide: `NEURAL_TRADER_CONFIGURATION_GUIDE.md`*