# Global Market Hours Reference Guide
*Production-Ready Reference for Neural Trader*

## Overview
This document provides comprehensive trading hours, holiday schedules, and timezone information for major global exchanges. All times are provided in both local exchange time and UTC/EST conversions for production use.

## Table of Contents
1. [Major Exchange Trading Hours](#major-exchange-trading-hours)
2. [2025 Holiday Calendars](#2025-holiday-calendars)
3. [Special Trading Days](#special-trading-days)
4. [Timezone Considerations](#timezone-considerations)
5. [Implementation Best Practices](#implementation-best-practices)

---

## Major Exchange Trading Hours

### New York Stock Exchange (NYSE) & NASDAQ
- **Location**: New York, USA
- **Timezone**: Eastern Time (ET) - UTC-5 (EST) / UTC-4 (EDT)
- **Regular Hours**: 9:30 AM - 4:00 PM ET
- **Pre-Market**: 4:00 AM - 9:30 AM ET
- **After-Hours**: 4:00 PM - 8:00 PM ET
- **Currency**: USD
- **Important Note**: NYSE Arca planning to extend to 22-hour trading in 2025 (pending approval)

### London Stock Exchange (LSE)
- **Location**: London, UK
- **Timezone**: GMT/BST - UTC+0 (GMT) / UTC+1 (BST)
- **Regular Hours**: 8:00 AM - 4:30 PM GMT/BST
- **Pre-Trading**: 7:00 AM - 8:00 AM
- **Post-Trading**: 4:30 PM - 5:00 PM
- **Currency**: GBP
- **Auction Times**: Opening auction 7:50 AM - 8:00 AM, Closing auction 4:30 PM - 4:35 PM

### Tokyo Stock Exchange (TSE/JPX)
- **Location**: Tokyo, Japan
- **Timezone**: Japan Standard Time (JST) - UTC+9 (no DST)
- **Morning Session**: 9:00 AM - 11:30 AM JST
- **Lunch Break**: 11:30 AM - 12:30 PM JST
- **Afternoon Session**: 12:30 PM - 3:00 PM JST (3:25 PM for some securities)
- **Currency**: JPY
- **Night Session**: Derivatives only, until 5:30 AM next day

### Frankfurt Stock Exchange (FSE/Xetra)
- **Location**: Frankfurt, Germany
- **Timezone**: Central European Time (CET/CEST) - UTC+1 (CET) / UTC+2 (CEST)
- **Xetra Electronic Trading**: 9:00 AM - 5:30 PM CET
- **Frankfurt Floor Trading**: 8:00 AM - 10:00 PM CET
- **Currency**: EUR
- **Opening Auction**: 8:50 AM - 9:00 AM
- **Closing Auction**: 5:30 PM - 5:35 PM

### Hong Kong Stock Exchange (HKEX)
- **Location**: Hong Kong
- **Timezone**: Hong Kong Time (HKT) - UTC+8 (no DST)
- **Morning Session**: 9:30 AM - 12:00 PM HKT
- **Lunch Break**: 12:00 PM - 1:00 PM HKT
- **Afternoon Session**: 1:00 PM - 4:00 PM HKT
- **Currency**: HKD
- **Pre-Opening**: 9:00 AM - 9:30 AM

---

## 2025 Holiday Calendars

### NYSE & NASDAQ Holidays 2025
| Date | Holiday | Notes |
|------|---------|-------|
| January 1 (Wed) | New Year's Day | Closed |
| January 20 (Mon) | Martin Luther King Jr. Day | Closed |
| February 17 (Mon) | Presidents Day | Closed |
| April 18 (Fri) | Good Friday | Closed |
| May 26 (Mon) | Memorial Day | Closed |
| June 19 (Thu) | Juneteenth | Closed |
| July 4 (Fri) | Independence Day | Closed |
| September 1 (Mon) | Labor Day | Closed |
| November 27 (Thu) | Thanksgiving Day | Closed |
| December 25 (Thu) | Christmas Day | Closed |

#### Early Closures (1:00 PM ET)
- July 3 (Thu) - Day before Independence Day
- November 28 (Fri) - Day after Thanksgiving
- December 24 (Wed) - Christmas Eve

### LSE Holidays 2025
| Date | Holiday | Notes |
|------|---------|-------|
| January 1 | New Year's Day | Closed |
| April 18 | Good Friday | Closed |
| April 21 | Easter Monday | Closed |
| May 5 | Early May Bank Holiday | Closed |
| May 26 | Spring Bank Holiday | Closed |
| August 25 | Summer Bank Holiday | Closed |
| December 24 | Christmas Eve | Early close |
| December 25 | Christmas Day | Closed |
| December 26 | Boxing Day | Closed |

### TSE/JPX Holidays 2025
| Date | Holiday | Notes |
|------|---------|-------|
| January 1-3 | New Year Holidays | Closed |
| January 13 (Mon) | Coming of Age Day | Closed (BCP testing) |
| February 11 (Tue) | National Foundation Day | Closed |
| February 24 (Mon) | Emperor's Birthday (substitute) | Closed |
| March 20 (Thu) | Vernal Equinox | Closed |
| April 29 (Tue) | Showa Day | Closed |
| May 5 (Mon) | Children's Day | Closed |
| May 6 (Tue) | Greenery Day (substitute) | Closed |
| July 21 (Mon) | Marine Day | Closed |
| August 11 (Mon) | Mountain Day | Closed |
| September 15 (Mon) | Respect for the Aged Day | Closed (BCP testing) |
| September 23 (Tue) | Autumnal Equinox | Closed |
| October 13 (Mon) | Sports Day | Closed |
| November 3 (Mon) | Culture Day | Closed |
| November 24 (Mon) | Labor Thanksgiving Day (substitute) | Closed |
| December 31 | Market Holiday | Closed |

### Xetra/FSE Holidays 2025
Germany has fewer market holidays than other exchanges. Notable closures include:
- New Year's Day
- Good Friday
- Easter Monday
- May 1 (Labor Day)
- December 24 (Christmas Eve) - Shortened trading
- December 25-26 (Christmas/Boxing Day)
- December 31 (New Year's Eve)

*Note: Xetra remains open on many German public holidays like Whit Monday and Ascension Day*

### HKEX Holidays 2025
*Note: Complete 2025 calendar requires verification from official HKEX sources*
- Follows Hong Kong public holidays
- Includes Chinese New Year (multi-day closure)
- Mid-Autumn Festival
- National Day holidays

---

## Special Trading Days

### Half-Day Trading Sessions
- **NYSE/NASDAQ**: 1:00 PM ET close on designated days
- **LSE**: Christmas Eve early closure
- **HKEX**: Various Hong Kong holidays may have noon closures
- **Xetra**: Christmas Eve shortened hours (9:30 AM - 1:00 PM CET)

### Extended Trading Considerations
- **US Markets**: Pre-market from 4:00 AM ET, after-hours until 8:00 PM ET
- **Xetra**: Floor trading continues until 10:00 PM CET on Frankfurt exchange
- **TSE**: Night sessions for derivatives markets

---

## Timezone Considerations

### Daylight Saving Time (DST) Transitions 2025
- **US (EDT/EST)**: 
  - Spring forward: March 9, 2025
  - Fall back: November 2, 2025
- **UK (BST/GMT)**:
  - Spring forward: March 30, 2025
  - Fall back: October 26, 2025
- **EU (CEST/CET)**:
  - Spring forward: March 30, 2025
  - Fall back: October 26, 2025
- **Japan, Hong Kong**: No DST observed

### Trading Hours Overlap (in UTC)
- **London-Frankfurt**: 7:00-16:30 UTC (winter) / 6:00-15:30 UTC (summer)
- **London-New York**: 14:30-16:30 UTC (winter) / 13:30-15:30 UTC (summer)
- **Tokyo-Hong Kong**: 00:00-07:00 UTC
- **Frankfurt-New York**: 14:30-16:30 UTC (winter) / 13:30-15:30 UTC (summer)

---

## Implementation Best Practices

### 1. Market Status Detection
```python
def is_market_open(exchange: str, timestamp: datetime) -> bool:
    """
    Check if market is open at given timestamp
    - Convert timestamp to exchange local time
    - Check against regular hours
    - Verify not a holiday
    - Account for special sessions
    """
    pass
```

### 2. Holiday Calendar Management
- Store holidays in a structured format (JSON/Database)
- Include both fixed and observed dates
- Update annually from official exchange sources
- Cache holiday lookups for performance

### 3. Timezone Handling
- Always store times in UTC internally
- Convert to local exchange time for display
- Use reliable timezone libraries (pytz, zoneinfo)
- Handle DST transitions gracefully

### 4. Special Considerations
- **Asian Markets**: Account for lunch breaks
- **US Markets**: Consider pre/after-hours trading
- **European Markets**: Note different holiday observances by country
- **Global Coordination**: Plan for no-overlap periods

### 5. Data Quality Checks
- Validate against multiple sources
- Monitor for holiday announcements
- Track special market closures (weather, emergencies)
- Implement fallback mechanisms

### 6. Performance Optimization
- Pre-calculate market sessions for common queries
- Cache timezone conversions
- Use efficient data structures for holiday lookups
- Implement lazy loading for historical data

### 7. Production Monitoring
- Alert on unexpected market closures
- Track API availability during market hours
- Monitor data feed latency
- Log timezone-related issues

---

## Resources and References

### Official Exchange Websites
- **NYSE**: https://www.nyse.com/markets/hours-calendars
- **NASDAQ**: https://www.nasdaq.com/market-activity/stock-market-holiday-schedule
- **LSE**: https://www.londonstockexchange.com/equities-trading/business-days
- **JPX**: https://www.jpx.co.jp/english/corporate/about-jpx/calendar/
- **Xetra**: https://www.xetra.com/xetra-en/newsroom/trading-calendar
- **HKEX**: https://www.hkex.com.hk/News/HKEX-Calendar

### Data Sources
- TradingHours.com - Comprehensive market hours database
- Official exchange circulars and notices
- Financial data providers (Bloomberg, Reuters, etc.)

### Update Schedule
- Review quarterly for any changes
- Verify annually in Q4 for next year's calendar
- Monitor for special announcements

---

*Last Updated: July 29, 2025*
*Version: 1.0.0*