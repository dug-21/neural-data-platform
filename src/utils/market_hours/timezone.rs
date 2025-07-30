//! Timezone handling and conversion utilities
//! 
//! Provides timezone conversion functionality for market hours calculations
//! without external dependencies on chrono-tz.

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::utils::market_hours::exchanges::Exchange;

/// Timezone conversion utilities
pub struct TimezoneConverter {
    exchange_offsets: HashMap<Exchange, i32>,
}

impl TimezoneConverter {
    pub fn new() -> Self {
        let mut exchange_offsets = HashMap::new();
        
        // Americas
        exchange_offsets.insert(Exchange::NYSE, -5);       // EST
        exchange_offsets.insert(Exchange::NASDAQ, -5);     // EST
        exchange_offsets.insert(Exchange::TORONTO, -5);    // EST
        exchange_offsets.insert(Exchange::MEXICO, -6);     // CST
        exchange_offsets.insert(Exchange::SAOPAULO, -3);   // BRT
        exchange_offsets.insert(Exchange::BUENOSAIRES, -3); // ART
        exchange_offsets.insert(Exchange::SANTIAGO, -4);   // CLT
        
        // Europe
        exchange_offsets.insert(Exchange::LSE, 0);         // GMT
        exchange_offsets.insert(Exchange::FRANKFURT, 1);   // CET
        exchange_offsets.insert(Exchange::PARIS, 1);       // CET
        exchange_offsets.insert(Exchange::MILAN, 1);       // CET
        exchange_offsets.insert(Exchange::MADRID, 1);      // CET
        exchange_offsets.insert(Exchange::AMSTERDAM, 1);   // CET
        exchange_offsets.insert(Exchange::ZURICH, 1);      // CET
        exchange_offsets.insert(Exchange::STOCKHOLM, 1);   // CET
        exchange_offsets.insert(Exchange::OSLO, 1);        // CET
        exchange_offsets.insert(Exchange::COPENHAGEN, 1);  // CET
        exchange_offsets.insert(Exchange::HELSINKI, 2);    // EET
        exchange_offsets.insert(Exchange::MOSCOW, 3);      // MSK
        
        // Asia-Pacific
        exchange_offsets.insert(Exchange::TSE, 9);         // JST
        exchange_offsets.insert(Exchange::SSE, 8);         // CST
        exchange_offsets.insert(Exchange::BSE, 5);         // IST (5:30, but simplified to 5)
        exchange_offsets.insert(Exchange::HKEX, 8);        // HKT
        exchange_offsets.insert(Exchange::SINGAPORE, 8);   // SGT
        exchange_offsets.insert(Exchange::SEOUL, 9);       // KST
        exchange_offsets.insert(Exchange::TAIWAN, 8);      // CST
        exchange_offsets.insert(Exchange::SYDNEY, 10);     // AEST
        exchange_offsets.insert(Exchange::WELLINGTON, 12); // NZST
        exchange_offsets.insert(Exchange::BANGKOK, 7);     // ICT
        exchange_offsets.insert(Exchange::JAKARTA, 7);     // WIB
        exchange_offsets.insert(Exchange::KUALALUMPUR, 8); // MYT
        
        // Africa
        exchange_offsets.insert(Exchange::JOHANNESBURG, 2); // SAST
        
        Self {
            exchange_offsets,
        }
    }

    /// Convert UTC time to exchange local time
    pub fn convert_to_exchange_time(&self, utc_time: DateTime<Utc>, exchange: Exchange) -> DateTime<Utc> {
        if let Some(offset) = self.exchange_offsets.get(&exchange) {
            utc_time + Duration::hours(*offset as i64)
        } else {
            utc_time
        }
    }

    /// Convert exchange local time to UTC
    pub fn convert_to_utc(&self, local_time: DateTime<Utc>, exchange: Exchange) -> DateTime<Utc> {
        if let Some(offset) = self.exchange_offsets.get(&exchange) {
            local_time - Duration::hours(*offset as i64)
        } else {
            local_time
        }
    }

    /// Get timezone offset for an exchange
    pub fn get_offset(&self, exchange: Exchange) -> Option<i32> {
        self.exchange_offsets.get(&exchange).copied()
    }

    /// Get timezone name for an exchange
    pub fn get_timezone_name(&self, exchange: Exchange) -> &'static str {
        match exchange {
            // Americas
            Exchange::NYSE | Exchange::NASDAQ | Exchange::TORONTO => "America/New_York",
            Exchange::MEXICO => "America/Mexico_City",
            Exchange::SAOPAULO => "America/Sao_Paulo",
            Exchange::BUENOSAIRES => "America/Buenos_Aires",
            Exchange::SANTIAGO => "America/Santiago",
            
            // Europe
            Exchange::LSE => "Europe/London",
            Exchange::FRANKFURT => "Europe/Berlin",
            Exchange::PARIS => "Europe/Paris",
            Exchange::MILAN => "Europe/Rome",
            Exchange::MADRID => "Europe/Madrid",
            Exchange::AMSTERDAM => "Europe/Amsterdam",
            Exchange::ZURICH => "Europe/Zurich",
            Exchange::STOCKHOLM => "Europe/Stockholm",
            Exchange::OSLO => "Europe/Oslo",
            Exchange::COPENHAGEN => "Europe/Copenhagen",
            Exchange::HELSINKI => "Europe/Helsinki",
            Exchange::MOSCOW => "Europe/Moscow",
            
            // Asia-Pacific
            Exchange::TSE => "Asia/Tokyo",
            Exchange::SSE => "Asia/Shanghai",
            Exchange::BSE => "Asia/Kolkata",
            Exchange::HKEX => "Asia/Hong_Kong",
            Exchange::SINGAPORE => "Asia/Singapore",
            Exchange::SEOUL => "Asia/Seoul",
            Exchange::TAIWAN => "Asia/Taipei",
            Exchange::SYDNEY => "Australia/Sydney",
            Exchange::WELLINGTON => "Pacific/Auckland",
            Exchange::BANGKOK => "Asia/Bangkok",
            Exchange::JAKARTA => "Asia/Jakarta",
            Exchange::KUALALUMPUR => "Asia/Kuala_Lumpur",
            
            // Africa
            Exchange::JOHANNESBURG => "Africa/Johannesburg",
            
            // Custom
            Exchange::CUSTOM => "UTC",
        }
    }

    /// Check if daylight saving time is in effect (simplified)
    /// Note: This is a simplified implementation. In production, you'd want
    /// to use proper timezone libraries for accurate DST calculations.
    pub fn is_dst_active(&self, exchange: Exchange, time: DateTime<Utc>) -> bool {
        let local_time = self.convert_to_exchange_time(time, exchange);
        let month = local_time.month();
        let day = local_time.day();
        
        match exchange {
            // US exchanges (second Sunday in March to first Sunday in November)
            Exchange::NYSE | Exchange::NASDAQ | Exchange::TORONTO => {
                (month > 3 && month < 11) || 
                (month == 3 && day > 14) || 
                (month == 11 && day <= 7)
            },
            
            // European exchanges (last Sunday in March to last Sunday in October)
            Exchange::LSE | Exchange::FRANKFURT | Exchange::PARIS | 
            Exchange::MILAN | Exchange::MADRID | Exchange::AMSTERDAM |
            Exchange::ZURICH | Exchange::STOCKHOLM | Exchange::OSLO |
            Exchange::COPENHAGEN => {
                (month > 3 && month < 10) ||
                (month == 3 && day > 25) ||
                (month == 10 && day <= 25)
            },
            
            // Southern hemisphere (Australia/New Zealand) - opposite of northern
            Exchange::SYDNEY | Exchange::WELLINGTON => {
                month < 4 || month > 9
            },
            
            // Most other exchanges don't observe DST or have complex rules
            _ => false,
        }
    }

    /// Get adjusted offset accounting for daylight saving time
    pub fn get_adjusted_offset(&self, exchange: Exchange, time: DateTime<Utc>) -> i32 {
        let base_offset = self.get_offset(exchange).unwrap_or(0);
        
        if self.is_dst_active(exchange, time) {
            match exchange {
                // Add 1 hour for DST
                Exchange::NYSE | Exchange::NASDAQ | Exchange::TORONTO |
                Exchange::LSE | Exchange::FRANKFURT | Exchange::PARIS |
                Exchange::MILAN | Exchange::MADRID | Exchange::AMSTERDAM |
                Exchange::ZURICH | Exchange::STOCKHOLM | Exchange::OSLO |
                Exchange::COPENHAGEN | Exchange::SYDNEY | Exchange::WELLINGTON => {
                    base_offset + 1
                },
                _ => base_offset,
            }
        } else {
            base_offset
        }
    }

    /// Convert time with DST adjustment
    pub fn convert_with_dst(&self, utc_time: DateTime<Utc>, exchange: Exchange) -> DateTime<Utc> {
        let adjusted_offset = self.get_adjusted_offset(exchange, utc_time);
        utc_time + Duration::hours(adjusted_offset as i64)
    }

    /// Get all supported exchanges and their current offsets
    pub fn get_all_offsets(&self, reference_time: DateTime<Utc>) -> HashMap<Exchange, i32> {
        self.exchange_offsets
            .keys()
            .map(|&exchange| {
                let adjusted_offset = self.get_adjusted_offset(exchange, reference_time);
                (exchange, adjusted_offset)
            })
            .collect()
    }

    /// Calculate time difference between two exchanges
    pub fn time_difference(&self, exchange1: Exchange, exchange2: Exchange, reference_time: DateTime<Utc>) -> i32 {
        let offset1 = self.get_adjusted_offset(exchange1, reference_time);
        let offset2 = self.get_adjusted_offset(exchange2, reference_time);
        offset1 - offset2
    }

    /// Find exchanges in the same timezone
    pub fn exchanges_in_timezone(&self, target_exchange: Exchange, reference_time: DateTime<Utc>) -> Vec<Exchange> {
        let target_offset = self.get_adjusted_offset(target_exchange, reference_time);
        
        self.exchange_offsets
            .keys()
            .filter(|&&exchange| {
                self.get_adjusted_offset(exchange, reference_time) == target_offset
            })
            .copied()
            .collect()
    }

    /// Get business hours overlap between two exchanges
    pub fn get_overlap_hours(&self, exchange1: Exchange, exchange2: Exchange, reference_time: DateTime<Utc>) -> Option<(i32, i32)> {
        // Simplified calculation assuming 9:00-17:00 business hours
        let offset1 = self.get_adjusted_offset(exchange1, reference_time);
        let offset2 = self.get_adjusted_offset(exchange2, reference_time);
        
        // Convert to UTC hours (0-23)
        let start1 = (9 - offset1).rem_euclid(24);
        let end1 = (17 - offset1).rem_euclid(24);
        let start2 = (9 - offset2).rem_euclid(24);
        let end2 = (17 - offset2).rem_euclid(24);
        
        // Find overlap
        let overlap_start = start1.max(start2);
        let overlap_end = end1.min(end2);
        
        if overlap_start < overlap_end {
            Some((overlap_start, overlap_end))
        } else {
            None
        }
    }
}

impl Default for TimezoneConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// Timezone-aware time calculations
pub struct TimeCalculator {
    converter: TimezoneConverter,
}

impl TimeCalculator {
    pub fn new() -> Self {
        Self {
            converter: TimezoneConverter::new(),
        }
    }

    /// Get the current market time for an exchange
    pub fn market_time(&self, exchange: Exchange) -> DateTime<Utc> {
        self.converter.convert_with_dst(Utc::now(), exchange)
    }

    /// Check if a given UTC time falls within business hours for an exchange
    pub fn is_business_hours(&self, utc_time: DateTime<Utc>, exchange: Exchange) -> bool {
        let local_time = self.converter.convert_with_dst(utc_time, exchange);
        let hour = local_time.hour();
        
        // Most exchanges trade 9:00-17:00 local time (simplified)
        hour >= 9 && hour < 17
    }

    /// Get next business day opening time for an exchange
    pub fn next_business_open(&self, exchange: Exchange) -> DateTime<Utc> {
        let mut current = Utc::now();
        
        loop {
            let local_time = self.converter.convert_with_dst(current, exchange);
            let weekday = local_time.weekday();
            
            // Skip weekends
            if weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun {
                current = current + Duration::days(1);
                continue;
            }
            
            // If before 9 AM local time, return today's opening
            if local_time.hour() < 9 {
                let opening_local = local_time
                    .date_naive()
                    .and_hms_opt(9, 0, 0)
                    .unwrap()
                    .and_local_timezone(Utc)
                    .unwrap();
                
                return self.converter.convert_to_utc(opening_local, exchange);
            }
            
            // Otherwise, next business day
            current = current + Duration::days(1);
        }
    }
}

impl Default for TimeCalculator {
    fn default() -> Self {
        Self::new()
    }
}