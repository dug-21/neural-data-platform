import { MarketDataRow, TradeDataRow } from './csvParser';
import { DateTime } from 'luxon';

// Import the TimeSeriesData type from the main data module
export interface TimeSeriesData {
  symbol: string;
  timestamp: Date;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  indicators: Record<string, number>;
  source?: string;
  entity?: string;
  value?: number;
  metadata?: any;
}

export interface TransformOptions {
  symbol: string;
  timezone?: string;
  validateData?: boolean;
  includeMetadata?: boolean;
}

export interface ValidationResult {
  isValid: boolean;
  errors: string[];
}

/**
 * Transforms Polygon CSV data to internal TimeSeriesData format
 */
export class PolygonDataTransformer {
  private symbol: string;
  private timezone: string;
  private validateData: boolean;
  private includeMetadata: boolean;
  private validationErrors: Map<string, number> = new Map();

  constructor(options: TransformOptions) {
    this.symbol = options.symbol;
    this.timezone = options.timezone || 'America/New_York';
    this.validateData = options.validateData !== false;
    this.includeMetadata = options.includeMetadata !== false;
  }

  /**
   * Transform a batch of market data rows
   */
  transformMarketDataBatch(rows: MarketDataRow[]): TimeSeriesData[] {
    const transformed: TimeSeriesData[] = [];
    
    for (const row of rows) {
      try {
        const data = this.transformMarketDataRow(row);
        
        if (this.validateData) {
          const validation = this.validateTimeSeriesData(data);
          if (!validation.isValid) {
            this.recordValidationErrors(validation.errors);
            continue; // Skip invalid data
          }
        }
        
        transformed.push(data);
      } catch (error) {
        console.error(`Error transforming row:`, error, row);
      }
    }
    
    return transformed;
  }

  /**
   * Transform a batch of trade data rows
   */
  transformTradeDataBatch(rows: TradeDataRow[]): TimeSeriesData[] {
    const transformed: TimeSeriesData[] = [];
    
    for (const row of rows) {
      try {
        const data = this.transformTradeDataRow(row);
        
        if (this.validateData) {
          const validation = this.validateTimeSeriesData(data);
          if (!validation.isValid) {
            this.recordValidationErrors(validation.errors);
            continue;
          }
        }
        
        transformed.push(data);
      } catch (error) {
        console.error(`Error transforming trade row:`, error, row);
      }
    }
    
    return transformed;
  }

  /**
   * Transform a single market data row
   */
  private transformMarketDataRow(row: MarketDataRow): TimeSeriesData {
    // Convert timestamp from milliseconds to Date
    const timestamp = DateTime.fromMillis(row.timestamp, { zone: this.timezone })
      .toUTC()
      .toJSDate();
    
    const data: TimeSeriesData = {
      symbol: this.symbol,
      timestamp,
      open: row.open,
      high: row.high,
      low: row.low,
      close: row.close,
      volume: row.volume,
      indicators: {},
      source: 'polygon',
      entity: this.symbol,
      value: row.close
    };

    // Add VWAP and transaction count to indicators if available
    if (row.vwap !== undefined) {
      data.indicators.vwap = row.vwap;
    }
    
    if (row.transactions !== undefined) {
      data.indicators.transactions = row.transactions;
    }

    // Add metadata if requested
    if (this.includeMetadata) {
      data.metadata = {
        provider: 'polygon',
        dataType: 'aggregates',
        originalTimestamp: row.timestamp,
        timezone: this.timezone
      };
    }

    return data;
  }

  /**
   * Transform a single trade data row
   */
  private transformTradeDataRow(row: TradeDataRow): TimeSeriesData {
    // Convert timestamp from milliseconds to Date
    const timestamp = DateTime.fromMillis(row.timestamp, { zone: this.timezone })
      .toUTC()
      .toJSDate();
    
    const data: TimeSeriesData = {
      symbol: this.symbol,
      timestamp,
      open: row.price,  // For trades, all prices are the same
      high: row.price,
      low: row.price,
      close: row.price,
      volume: row.size,
      indicators: {},
      source: 'polygon',
      entity: this.symbol,
      value: row.price
    };

    // Add trade-specific indicators
    if (row.conditions && row.conditions.length > 0) {
      data.indicators.conditionCount = row.conditions.length;
    }
    
    if (row.exchange) {
      data.indicators.exchange = parseInt(row.exchange) || 0;
    }

    // Add metadata if requested
    if (this.includeMetadata) {
      data.metadata = {
        provider: 'polygon',
        dataType: 'trades',
        originalTimestamp: row.timestamp,
        timezone: this.timezone,
        tradeConditions: row.conditions,
        exchange: row.exchange
      };
    }

    return data;
  }

  /**
   * Validate time series data
   */
  validateTimeSeriesData(data: TimeSeriesData): ValidationResult {
    const errors: string[] = [];

    // Price validation
    if (data.open <= 0) {
      errors.push('Open price must be positive');
    }
    if (data.high <= 0) {
      errors.push('High price must be positive');
    }
    if (data.low <= 0) {
      errors.push('Low price must be positive');
    }
    if (data.close <= 0) {
      errors.push('Close price must be positive');
    }

    // OHLC relationship validation
    if (data.high < data.low) {
      errors.push(`High (${data.high}) cannot be less than low (${data.low})`);
    }
    if (data.high < data.open) {
      errors.push(`High (${data.high}) cannot be less than open (${data.open})`);
    }
    if (data.high < data.close) {
      errors.push(`High (${data.high}) cannot be less than close (${data.close})`);
    }
    if (data.low > data.open) {
      errors.push(`Low (${data.low}) cannot be greater than open (${data.open})`);
    }
    if (data.low > data.close) {
      errors.push(`Low (${data.low}) cannot be greater than close (${data.close})`);
    }

    // Volume validation
    if (data.volume < 0) {
      errors.push('Volume cannot be negative');
    }

    // Extreme price movement check (>50% in a minute)
    const priceChange = Math.abs(data.close - data.open) / data.open;
    if (priceChange > 0.5) {
      errors.push(`Extreme price movement detected: ${(priceChange * 100).toFixed(2)}%`);
    }

    // Timestamp validation
    if (!data.timestamp || isNaN(data.timestamp.getTime())) {
      errors.push('Invalid timestamp');
    }

    // Future timestamp check
    if (data.timestamp > new Date()) {
      errors.push('Timestamp is in the future');
    }

    return {
      isValid: errors.length === 0,
      errors
    };
  }

  /**
   * Record validation errors for reporting
   */
  private recordValidationErrors(errors: string[]): void {
    for (const error of errors) {
      const count = this.validationErrors.get(error) || 0;
      this.validationErrors.set(error, count + 1);
    }
  }

  /**
   * Get validation error summary
   */
  getValidationSummary(): Map<string, number> {
    return new Map(this.validationErrors);
  }

  /**
   * Reset validation error counts
   */
  resetValidationErrors(): void {
    this.validationErrors.clear();
  }

  /**
   * Aggregate minute data to larger timeframes
   */
  aggregateToTimeframe(
    data: TimeSeriesData[],
    timeframe: '5min' | '15min' | '30min' | '1hour' | '1day'
  ): TimeSeriesData[] {
    if (data.length === 0) return [];

    // Sort by timestamp
    const sorted = [...data].sort((a, b) => 
      a.timestamp.getTime() - b.timestamp.getTime()
    );

    const aggregated: TimeSeriesData[] = [];
    const bucketSize = this.getTimeframeMiniutes(timeframe);
    
    let currentBucket: TimeSeriesData[] = [];
    let bucketStart: Date | null = null;

    for (const item of sorted) {
      const itemTime = DateTime.fromJSDate(item.timestamp);
      
      if (!bucketStart) {
        bucketStart = this.getBucketStart(itemTime, bucketSize).toJSDate();
      }

      const currentBucketStart = this.getBucketStart(itemTime, bucketSize).toJSDate();
      
      if (currentBucketStart.getTime() !== bucketStart.getTime() && currentBucket.length > 0) {
        // Process current bucket
        aggregated.push(this.aggregateBucket(currentBucket, bucketStart));
        
        // Start new bucket
        currentBucket = [item];
        bucketStart = currentBucketStart;
      } else {
        currentBucket.push(item);
      }
    }

    // Process final bucket
    if (currentBucket.length > 0 && bucketStart) {
      aggregated.push(this.aggregateBucket(currentBucket, bucketStart));
    }

    return aggregated;
  }

  /**
   * Get timeframe in minutes
   */
  private getTimeframeMiniutes(timeframe: string): number {
    const map: Record<string, number> = {
      '5min': 5,
      '15min': 15,
      '30min': 30,
      '1hour': 60,
      '1day': 1440
    };
    return map[timeframe] || 60;
  }

  /**
   * Get bucket start time
   */
  private getBucketStart(time: DateTime, bucketMinutes: number): DateTime {
    const totalMinutes = time.hour * 60 + time.minute;
    const bucketIndex = Math.floor(totalMinutes / bucketMinutes);
    const bucketStartMinutes = bucketIndex * bucketMinutes;
    
    return time.startOf('day').plus({ minutes: bucketStartMinutes });
  }

  /**
   * Aggregate data within a bucket
   */
  private aggregateBucket(bucket: TimeSeriesData[], bucketStart: Date): TimeSeriesData {
    const open = bucket[0].open;
    const close = bucket[bucket.length - 1].close;
    const high = Math.max(...bucket.map(d => d.high));
    const low = Math.min(...bucket.map(d => d.low));
    const volume = bucket.reduce((sum, d) => sum + d.volume, 0);
    
    // Calculate VWAP if available
    let vwap = 0;
    let totalVolumeValue = 0;
    
    for (const item of bucket) {
      const itemVwap = item.indicators.vwap || item.close;
      totalVolumeValue += itemVwap * item.volume;
    }
    
    if (volume > 0) {
      vwap = totalVolumeValue / volume;
    }

    return {
      symbol: this.symbol,
      timestamp: bucketStart,
      open,
      high,
      low,
      close,
      volume,
      indicators: {
        vwap,
        dataPoints: bucket.length,
        transactions: bucket.reduce((sum, d) => 
          sum + (d.indicators.transactions || 0), 0
        )
      },
      source: 'polygon',
      entity: this.symbol,
      value: close,
      metadata: this.includeMetadata ? {
        provider: 'polygon',
        dataType: 'aggregated',
        originalDataPoints: bucket.length,
        aggregationTimeframe: `${this.getTimeframeMiniutes}min`
      } : undefined
    };
  }
}