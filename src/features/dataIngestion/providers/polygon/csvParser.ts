import { createGunzip } from 'zlib';
import { pipeline, Transform, Readable } from 'stream';
import * as csv from 'csv-parse';
import { promisify } from 'util';

const pipelineAsync = promisify(pipeline);

export interface MarketDataRow {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  vwap?: number;
  transactions?: number;
}

export interface TradeDataRow {
  timestamp: number;
  price: number;
  size: number;
  conditions?: string[];
  exchange?: string;
}

export interface ParseOptions {
  batchSize?: number;
  skipInvalid?: boolean;
  maxParseErrors?: number;
}

/**
 * Parser for Polygon's gzipped CSV files with streaming support
 */
export class GzippedCSVParser {
  private batchSize: number;
  private skipInvalid: boolean;
  private maxParseErrors: number;
  private parseErrors: number = 0;

  constructor(options: ParseOptions = {}) {
    this.batchSize = options.batchSize || 1000;
    this.skipInvalid = options.skipInvalid !== false;
    this.maxParseErrors = options.maxParseErrors || 100;
  }

  /**
   * Parse aggregate market data (OHLCV) from gzipped CSV stream
   */
  async *parseAggregatesStream(
    stream: Readable
  ): AsyncGenerator<MarketDataRow[], void, unknown> {
    const gunzip = createGunzip();
    const parser = csv.parse({
      columns: true,
      skip_empty_lines: true,
      relax_quotes: true,
      trim: true,
      cast: false, // We'll handle casting manually for better error handling
    });

    let batch: MarketDataRow[] = [];
    let rowCount = 0;
    let errorCount = 0;

    const transformer = new Transform({
      objectMode: true,
      transform: (record: any, encoding: string, callback: Function) => {
        try {
          rowCount++;
          const row = this.parseAggregateRow(record);
          
          if (row) {
            batch.push(row);
            
            if (batch.length >= this.batchSize) {
              this.push([...batch]);
              batch = [];
            }
          }
          
          callback();
        } catch (error) {
          errorCount++;
          
          if (errorCount > this.maxParseErrors) {
            callback(new Error(`Too many parse errors: ${errorCount}`));
          } else if (!this.skipInvalid) {
            callback(error);
          } else {
            // Skip invalid row and continue
            console.warn(`Skipping invalid row ${rowCount}:`, error.message);
            callback();
          }
        }
      },
      flush: (callback: Function) => {
        if (batch.length > 0) {
          this.push([...batch]);
        }
        callback();
      }
    });

    // Set up the pipeline
    stream
      .pipe(gunzip)
      .pipe(parser)
      .pipe(transformer);

    // Yield batches as they become available
    for await (const batch of transformer) {
      yield batch as MarketDataRow[];
    }

    // Log parsing statistics
    if (errorCount > 0) {
      console.warn(`Parsing completed with ${errorCount} errors out of ${rowCount} rows`);
    }
  }

  /**
   * Parse trade data from gzipped CSV stream
   */
  async *parseTradesStream(
    stream: Readable
  ): AsyncGenerator<TradeDataRow[], void, unknown> {
    const gunzip = createGunzip();
    const parser = csv.parse({
      columns: true,
      skip_empty_lines: true,
      relax_quotes: true,
      trim: true,
      cast: false,
    });

    let batch: TradeDataRow[] = [];
    let rowCount = 0;
    let errorCount = 0;

    const transformer = new Transform({
      objectMode: true,
      transform: (record: any, encoding: string, callback: Function) => {
        try {
          rowCount++;
          const row = this.parseTradeRow(record);
          
          if (row) {
            batch.push(row);
            
            if (batch.length >= this.batchSize) {
              this.push([...batch]);
              batch = [];
            }
          }
          
          callback();
        } catch (error) {
          errorCount++;
          
          if (errorCount > this.maxParseErrors) {
            callback(new Error(`Too many parse errors: ${errorCount}`));
          } else if (!this.skipInvalid) {
            callback(error);
          } else {
            console.warn(`Skipping invalid trade row ${rowCount}:`, error.message);
            callback();
          }
        }
      },
      flush: (callback: Function) => {
        if (batch.length > 0) {
          this.push([...batch]);
        }
        callback();
      }
    });

    // Set up the pipeline
    stream
      .pipe(gunzip)
      .pipe(parser)
      .pipe(transformer);

    // Yield batches as they become available
    for await (const batch of transformer) {
      yield batch as TradeDataRow[];
    }
  }

  /**
   * Parse a single aggregate row with validation
   */
  private parseAggregateRow(record: any): MarketDataRow | null {
    // Validate required fields
    if (!record.timestamp || !record.open || !record.high || 
        !record.low || !record.close || !record.volume) {
      throw new Error('Missing required fields in aggregate row');
    }

    // Parse timestamp
    const timestamp = this.parseTimestamp(record.timestamp);
    if (!timestamp || timestamp <= 0) {
      throw new Error(`Invalid timestamp: ${record.timestamp}`);
    }

    // Parse prices
    const open = parseFloat(record.open);
    const high = parseFloat(record.high);
    const low = parseFloat(record.low);
    const close = parseFloat(record.close);
    const volume = parseFloat(record.volume);

    // Validate prices
    if (isNaN(open) || isNaN(high) || isNaN(low) || isNaN(close) || isNaN(volume)) {
      throw new Error('Invalid numeric values in row');
    }

    if (open <= 0 || high <= 0 || low <= 0 || close <= 0) {
      throw new Error('Prices must be positive');
    }

    if (volume < 0) {
      throw new Error('Volume cannot be negative');
    }

    // Validate OHLC relationships
    if (high < low) {
      throw new Error(`High (${high}) cannot be less than low (${low})`);
    }

    if (high < open || high < close) {
      throw new Error('High must be >= open and close');
    }

    if (low > open || low > close) {
      throw new Error('Low must be <= open and close');
    }

    // Parse optional fields
    const vwap = record.vwap ? parseFloat(record.vwap) : undefined;
    const transactions = record.transactions ? parseInt(record.transactions) : undefined;

    return {
      timestamp,
      open,
      high,
      low,
      close,
      volume,
      vwap: vwap && !isNaN(vwap) ? vwap : undefined,
      transactions: transactions && !isNaN(transactions) ? transactions : undefined
    };
  }

  /**
   * Parse a single trade row with validation
   */
  private parseTradeRow(record: any): TradeDataRow | null {
    // Validate required fields
    if (!record.timestamp || !record.price || !record.size) {
      throw new Error('Missing required fields in trade row');
    }

    // Parse timestamp
    const timestamp = this.parseTimestamp(record.timestamp);
    if (!timestamp || timestamp <= 0) {
      throw new Error(`Invalid timestamp: ${record.timestamp}`);
    }

    // Parse price and size
    const price = parseFloat(record.price);
    const size = parseFloat(record.size);

    if (isNaN(price) || isNaN(size)) {
      throw new Error('Invalid numeric values in trade row');
    }

    if (price <= 0) {
      throw new Error('Price must be positive');
    }

    if (size <= 0) {
      throw new Error('Size must be positive');
    }

    // Parse conditions (JSON array)
    let conditions: string[] | undefined;
    if (record.conditions) {
      try {
        conditions = JSON.parse(record.conditions);
        if (!Array.isArray(conditions)) {
          conditions = undefined;
        }
      } catch {
        // If not valid JSON, treat as comma-separated string
        conditions = record.conditions.split(',').map((c: string) => c.trim());
      }
    }

    return {
      timestamp,
      price,
      size,
      conditions,
      exchange: record.exchange || undefined
    };
  }

  /**
   * Parse timestamp which could be in various formats
   */
  private parseTimestamp(value: string | number): number {
    // If already a number, assume it's milliseconds
    if (typeof value === 'number') {
      return value;
    }

    // Try parsing as integer (milliseconds)
    const asInt = parseInt(value);
    if (!isNaN(asInt)) {
      return asInt;
    }

    // Try parsing as ISO date
    const asDate = new Date(value).getTime();
    if (!isNaN(asDate)) {
      return asDate;
    }

    throw new Error(`Cannot parse timestamp: ${value}`);
  }

  /**
   * Get parsing statistics
   */
  getStatistics(): {
    parseErrors: number;
  } {
    return {
      parseErrors: this.parseErrors
    };
  }

  /**
   * Reset parser state
   */
  reset(): void {
    this.parseErrors = 0;
  }
}