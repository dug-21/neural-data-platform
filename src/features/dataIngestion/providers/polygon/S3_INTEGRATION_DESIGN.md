# Polygon S3 Flat File Integration Design

## Executive Summary

This document outlines the comprehensive design for integrating Polygon's S3-hosted flat files into the neural-trader system. The integration focuses on efficient downloading, parsing, and transformation of gzipped CSV files containing historical market data.

## S3 File Structure Overview

### Polygon Flat File Format

```
Bucket: polygon-flat-files
Region: us-east-1

File Structure:
├── stocks/
│   ├── {year}/
│   │   ├── {month}/
│   │   │   ├── {day}/
│   │   │   │   └── {ticker}_{date}.csv.gz
│   │   │   └── aggregates/
│   │   │       ├── minute/{ticker}_{date}.csv.gz
│   │   │       ├── hour/{ticker}_{date}.csv.gz
│   │   │       └── day/{ticker}_{date}.csv.gz
├── options/
│   └── {similar structure}
├── crypto/
│   └── {similar structure}
└── forex/
    └── {similar structure}
```

### CSV File Format (Gzipped)

**Minute Aggregates Format:**
```csv
timestamp,open,high,low,close,volume,vwap,transactions
1640995200000,420.12,420.45,419.80,420.22,125432,420.18,892
1640995260000,420.22,420.38,420.10,420.35,98234,420.25,743
```

**Trades Format:**
```csv
timestamp,price,size,conditions,exchange
1640995200123,420.12,100,["@","F"],4
1640995200456,420.13,200,["@"],11
```

## Architecture Components

### 1. S3 Client Wrapper

```typescript
import { S3Client, GetObjectCommand, ListObjectsV2Command } from '@aws-sdk/client-s3';
import { getSignedUrl } from '@aws-sdk/s3-request-presigner';
import { Readable } from 'stream';

export interface S3Config {
  bucketName: string;
  region: string;
  accessKeyId?: string;
  secretAccessKey?: string;
  endpoint?: string; // For S3-compatible services
  maxRetries?: number;
  requestTimeout?: number;
}

export class PolygonS3Client {
  private client: S3Client;
  private config: S3Config;

  constructor(config: S3Config) {
    this.config = {
      bucketName: 'polygon-flat-files',
      region: 'us-east-1',
      maxRetries: 3,
      requestTimeout: 30000,
      ...config
    };

    this.client = new S3Client({
      region: this.config.region,
      credentials: this.config.accessKeyId ? {
        accessKeyId: this.config.accessKeyId,
        secretAccessKey: this.config.secretAccessKey!
      } : undefined,
      maxAttempts: this.config.maxRetries,
      requestHandler: {
        requestTimeout: this.config.requestTimeout
      }
    });
  }

  async listFiles(prefix: string, maxKeys: number = 1000): Promise<S3FileInfo[]> {
    const files: S3FileInfo[] = [];
    let continuationToken: string | undefined;

    do {
      const command = new ListObjectsV2Command({
        Bucket: this.config.bucketName,
        Prefix: prefix,
        MaxKeys: maxKeys,
        ContinuationToken: continuationToken
      });

      const response = await this.client.send(command);
      
      if (response.Contents) {
        files.push(...response.Contents.map(obj => ({
          key: obj.Key!,
          size: obj.Size!,
          lastModified: obj.LastModified!,
          etag: obj.ETag
        })));
      }

      continuationToken = response.NextContinuationToken;
    } while (continuationToken);

    return files;
  }

  async downloadStream(key: string): Promise<Readable> {
    const command = new GetObjectCommand({
      Bucket: this.config.bucketName,
      Key: key
    });

    const response = await this.client.send(command);
    return response.Body as Readable;
  }

  async generatePresignedUrl(key: string, expiresIn: number = 3600): Promise<string> {
    const command = new GetObjectCommand({
      Bucket: this.config.bucketName,
      Key: key
    });

    return await getSignedUrl(this.client, command, { expiresIn });
  }
}
```

### 2. Gzipped CSV Parser

```typescript
import { createGunzip } from 'zlib';
import { pipeline } from 'stream/promises';
import * as csv from 'csv-parse';
import { Transform, Readable } from 'stream';

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

export class GzippedCSVParser {
  private batchSize: number;
  private parseOptions: csv.Options;

  constructor(batchSize: number = 1000) {
    this.batchSize = batchSize;
    this.parseOptions = {
      columns: true,
      cast: true,
      skip_empty_lines: true,
      relax_quotes: true,
      trim: true
    };
  }

  async *parseStream(stream: Readable): AsyncGenerator<MarketDataRow[], void, unknown> {
    const gunzip = createGunzip();
    const parser = csv.parse(this.parseOptions);
    
    let batch: MarketDataRow[] = [];
    
    const transformer = new Transform({
      objectMode: true,
      transform: (record, encoding, callback) => {
        try {
          const row = this.transformRecord(record);
          batch.push(row);
          
          if (batch.length >= this.batchSize) {
            this.push(batch);
            batch = [];
          }
          
          callback();
        } catch (error) {
          callback(error);
        }
      },
      flush: (callback) => {
        if (batch.length > 0) {
          this.push(batch);
        }
        callback();
      }
    });

    const pipelineStream = pipeline(
      stream,
      gunzip,
      parser,
      transformer
    );

    for await (const batch of transformer) {
      yield batch;
    }
  }

  private transformRecord(record: any): MarketDataRow {
    return {
      timestamp: parseInt(record.timestamp),
      open: parseFloat(record.open),
      high: parseFloat(record.high),
      low: parseFloat(record.low),
      close: parseFloat(record.close),
      volume: parseFloat(record.volume),
      vwap: record.vwap ? parseFloat(record.vwap) : undefined,
      transactions: record.transactions ? parseInt(record.transactions) : undefined
    };
  }
}
```

### 3. Data Transformer

```typescript
import { TimeSeriesData } from '../../../../data/mod';
import { DateTime } from 'luxon';

export class PolygonDataTransformer {
  private symbol: string;
  private timezone: string;

  constructor(symbol: string, timezone: string = 'America/New_York') {
    this.symbol = symbol;
    this.timezone = timezone;
  }

  transformBatch(rows: MarketDataRow[]): TimeSeriesData[] {
    return rows.map(row => this.transformRow(row));
  }

  private transformRow(row: MarketDataRow): TimeSeriesData {
    const timestamp = DateTime.fromMillis(row.timestamp, { zone: this.timezone });
    
    return {
      symbol: this.symbol,
      timestamp: timestamp.toUTC().toJSDate(),
      open: row.open,
      high: row.high,
      low: row.low,
      close: row.close,
      volume: row.volume,
      indicators: {
        vwap: row.vwap || 0,
        transactions: row.transactions || 0
      },
      source: 'polygon',
      entity: this.symbol,
      value: row.close,
      metadata: {
        provider: 'polygon',
        dataType: 'aggregates',
        originalTimestamp: row.timestamp
      }
    };
  }

  validateData(data: TimeSeriesData): boolean {
    // Price validation
    if (data.open <= 0 || data.high <= 0 || data.low <= 0 || data.close <= 0) {
      return false;
    }

    // OHLC relationship validation
    if (data.high < data.low || data.high < data.open || data.high < data.close) {
      return false;
    }

    if (data.low > data.open || data.low > data.close) {
      return false;
    }

    // Volume validation
    if (data.volume < 0) {
      return false;
    }

    // Extreme price movement check (>50% in a minute)
    const priceChange = Math.abs(data.close - data.open) / data.open;
    if (priceChange > 0.5) {
      return false;
    }

    return true;
  }
}
```

### 4. Concurrent Download Manager

```typescript
import { EventEmitter } from 'events';
import pLimit from 'p-limit';
import { backOff } from 'exponential-backoff';

export interface DownloadJob {
  id: string;
  symbol: string;
  date: string;
  s3Key: string;
  priority: number;
  retries: number;
  error?: Error;
}

export interface DownloadProgress {
  jobId: string;
  bytesDownloaded: number;
  totalBytes: number;
  recordsProcessed: number;
  percentComplete: number;
}

export class ConcurrentDownloadManager extends EventEmitter {
  private s3Client: PolygonS3Client;
  private parser: GzippedCSVParser;
  private concurrencyLimit: pLimit.Limit;
  private activeJobs: Map<string, DownloadJob>;
  private completedJobs: Set<string>;
  private failedJobs: Map<string, Error>;

  constructor(
    s3Client: PolygonS3Client,
    maxConcurrency: number = 10,
    batchSize: number = 5000
  ) {
    super();
    this.s3Client = s3Client;
    this.parser = new GzippedCSVParser(batchSize);
    this.concurrencyLimit = pLimit(maxConcurrency);
    this.activeJobs = new Map();
    this.completedJobs = new Set();
    this.failedJobs = new Map();
  }

  async downloadBatch(jobs: DownloadJob[]): Promise<Map<string, TimeSeriesData[]>> {
    // Sort jobs by priority
    const sortedJobs = [...jobs].sort((a, b) => b.priority - a.priority);
    
    const results = new Map<string, TimeSeriesData[]>();
    
    const downloadPromises = sortedJobs.map(job => 
      this.concurrencyLimit(() => this.downloadWithRetry(job, results))
    );

    await Promise.all(downloadPromises);

    return results;
  }

  private async downloadWithRetry(
    job: DownloadJob,
    results: Map<string, TimeSeriesData[]>
  ): Promise<void> {
    this.activeJobs.set(job.id, job);
    this.emit('jobStart', job);

    try {
      const data = await backOff(
        () => this.downloadSingleFile(job),
        {
          numOfAttempts: 3,
          startingDelay: 1000,
          timeMultiple: 2,
          maxDelay: 10000,
          jitter: 'full'
        }
      );

      results.set(job.id, data);
      this.completedJobs.add(job.id);
      this.emit('jobComplete', job, data.length);
    } catch (error) {
      this.failedJobs.set(job.id, error as Error);
      this.emit('jobFailed', job, error);
      throw error;
    } finally {
      this.activeJobs.delete(job.id);
    }
  }

  private async downloadSingleFile(job: DownloadJob): Promise<TimeSeriesData[]> {
    const stream = await this.s3Client.downloadStream(job.s3Key);
    const transformer = new PolygonDataTransformer(job.symbol);
    const allData: TimeSeriesData[] = [];
    
    let totalRecords = 0;
    let validRecords = 0;

    for await (const batch of this.parser.parseStream(stream)) {
      const transformedData = transformer.transformBatch(batch);
      
      for (const data of transformedData) {
        totalRecords++;
        if (transformer.validateData(data)) {
          allData.push(data);
          validRecords++;
        }
      }

      // Emit progress
      if (totalRecords % 10000 === 0) {
        this.emit('progress', {
          jobId: job.id,
          recordsProcessed: totalRecords,
          validRecords: validRecords
        });
      }
    }

    if (validRecords === 0) {
      throw new Error(`No valid records found in file: ${job.s3Key}`);
    }

    return allData;
  }

  getStatus(): {
    active: number;
    completed: number;
    failed: number;
    activeJobs: DownloadJob[];
  } {
    return {
      active: this.activeJobs.size,
      completed: this.completedJobs.size,
      failed: this.failedJobs.size,
      activeJobs: Array.from(this.activeJobs.values())
    };
  }
}
```

### 5. Smart Download Scheduler

```typescript
export interface SchedulerConfig {
  maxConcurrentDownloads: number;
  maxBandwidthMbps?: number;
  prioritySymbols?: string[];
  downloadTimeWindow?: {
    start: number; // Hour (0-23)
    end: number;   // Hour (0-23)
  };
}

export class SmartDownloadScheduler {
  private downloader: ConcurrentDownloadManager;
  private config: SchedulerConfig;
  private downloadHistory: DownloadMetrics[];
  private bandwidthMonitor: BandwidthMonitor;

  constructor(
    s3Client: PolygonS3Client,
    config: SchedulerConfig
  ) {
    this.config = config;
    this.downloader = new ConcurrentDownloadManager(
      s3Client,
      config.maxConcurrentDownloads
    );
    this.downloadHistory = [];
    this.bandwidthMonitor = new BandwidthMonitor();
  }

  async scheduleDownloads(
    symbols: string[],
    startDate: Date,
    endDate: Date
  ): Promise<void> {
    // Generate download jobs
    const jobs = await this.generateDownloadJobs(symbols, startDate, endDate);
    
    // Group jobs by priority and date
    const prioritizedJobs = this.prioritizeJobs(jobs);
    
    // Execute in optimized batches
    for (const batch of this.createOptimizedBatches(prioritizedJobs)) {
      // Check if within download window
      if (!this.isWithinDownloadWindow()) {
        await this.waitForDownloadWindow();
      }

      // Adjust concurrency based on bandwidth
      if (this.config.maxBandwidthMbps) {
        await this.adjustConcurrencyForBandwidth();
      }

      // Download batch
      const results = await this.downloader.downloadBatch(batch);
      
      // Store metrics
      this.recordDownloadMetrics(batch, results);
      
      // Emit progress
      this.emitSchedulerProgress();
    }
  }

  private async generateDownloadJobs(
    symbols: string[],
    startDate: Date,
    endDate: Date
  ): Promise<DownloadJob[]> {
    const jobs: DownloadJob[] = [];
    
    for (const symbol of symbols) {
      const dates = this.getDateRange(startDate, endDate);
      
      for (const date of dates) {
        const s3Key = this.buildS3Key(symbol, date);
        
        jobs.push({
          id: `${symbol}_${date}`,
          symbol,
          date,
          s3Key,
          priority: this.calculatePriority(symbol, date),
          retries: 0
        });
      }
    }

    return jobs;
  }

  private prioritizeJobs(jobs: DownloadJob[]): DownloadJob[] {
    return jobs.sort((a, b) => {
      // Priority symbols first
      if (this.config.prioritySymbols) {
        const aPriority = this.config.prioritySymbols.indexOf(a.symbol);
        const bPriority = this.config.prioritySymbols.indexOf(b.symbol);
        
        if (aPriority !== -1 && bPriority === -1) return -1;
        if (aPriority === -1 && bPriority !== -1) return 1;
        if (aPriority !== -1 && bPriority !== -1) {
          const diff = aPriority - bPriority;
          if (diff !== 0) return diff;
        }
      }

      // Recent dates first
      const dateA = new Date(a.date).getTime();
      const dateB = new Date(b.date).getTime();
      
      return dateB - dateA;
    });
  }

  private createOptimizedBatches(jobs: DownloadJob[]): DownloadJob[][] {
    const batches: DownloadJob[][] = [];
    const batchSize = this.config.maxConcurrentDownloads * 2; // Prepare next batch
    
    for (let i = 0; i < jobs.length; i += batchSize) {
      batches.push(jobs.slice(i, i + batchSize));
    }

    return batches;
  }

  private calculatePriority(symbol: string, date: string): number {
    let priority = 0;
    
    // Priority symbols get highest priority
    if (this.config.prioritySymbols?.includes(symbol)) {
      priority += 1000;
    }
    
    // Recent dates get higher priority
    const daysAgo = Math.floor(
      (Date.now() - new Date(date).getTime()) / (1000 * 60 * 60 * 24)
    );
    priority += Math.max(0, 365 - daysAgo);
    
    return priority;
  }
}
```

## Error Handling and Recovery

### Error Types and Strategies

```typescript
export enum ErrorType {
  NETWORK_ERROR = 'NETWORK_ERROR',
  S3_ACCESS_ERROR = 'S3_ACCESS_ERROR',
  PARSE_ERROR = 'PARSE_ERROR',
  VALIDATION_ERROR = 'VALIDATION_ERROR',
  STORAGE_ERROR = 'STORAGE_ERROR'
}

export class ErrorHandler {
  private errorCounts: Map<string, number> = new Map();
  private blacklistedFiles: Set<string> = new Set();
  
  async handleError(
    error: Error,
    context: {
      job: DownloadJob;
      errorType: ErrorType;
      canRetry: boolean;
    }
  ): Promise<boolean> {
    const errorKey = `${context.job.id}_${context.errorType}`;
    const errorCount = (this.errorCounts.get(errorKey) || 0) + 1;
    this.errorCounts.set(errorKey, errorCount);

    // Log error with context
    console.error(`Error processing ${context.job.id}:`, {
      error: error.message,
      type: context.errorType,
      attempts: errorCount,
      job: context.job
    });

    // Determine if should retry
    if (!context.canRetry || errorCount > 3) {
      this.blacklistedFiles.add(context.job.s3Key);
      return false;
    }

    // Different strategies per error type
    switch (context.errorType) {
      case ErrorType.NETWORK_ERROR:
        // Exponential backoff for network errors
        await this.sleep(Math.pow(2, errorCount) * 1000);
        return true;
        
      case ErrorType.S3_ACCESS_ERROR:
        // Check credentials and permissions
        if (error.message.includes('403')) {
          return false; // Don't retry permission errors
        }
        await this.sleep(5000);
        return true;
        
      case ErrorType.PARSE_ERROR:
        // Log for investigation, don't retry
        await this.logParseError(context.job, error);
        return false;
        
      case ErrorType.VALIDATION_ERROR:
        // Continue with partial data
        return false;
        
      default:
        return errorCount < 2;
    }
  }

  private async sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  private async logParseError(job: DownloadJob, error: Error): Promise<void> {
    // Log to file for manual investigation
    const logEntry = {
      timestamp: new Date().toISOString(),
      job,
      error: error.message,
      stack: error.stack
    };
    
    // In production, this would write to a proper logging system
    console.error('Parse error logged:', logEntry);
  }
}
```

## Performance Optimization

### 1. Connection Pooling

```typescript
export class S3ConnectionPool {
  private clients: PolygonS3Client[];
  private currentIndex: number = 0;
  
  constructor(poolSize: number = 5, config: S3Config) {
    this.clients = Array(poolSize)
      .fill(null)
      .map(() => new PolygonS3Client(config));
  }

  getClient(): PolygonS3Client {
    const client = this.clients[this.currentIndex];
    this.currentIndex = (this.currentIndex + 1) % this.clients.length;
    return client;
  }
}
```

### 2. Memory-Efficient Streaming

```typescript
export class MemoryEfficientProcessor {
  private maxMemoryMB: number;
  private currentMemoryUsage: number = 0;
  
  constructor(maxMemoryMB: number = 1024) {
    this.maxMemoryMB = maxMemoryMB;
  }

  async processLargeFile(
    stream: Readable,
    processor: (batch: TimeSeriesData[]) => Promise<void>
  ): Promise<void> {
    const parser = new GzippedCSVParser(1000);
    const transformer = new PolygonDataTransformer('TEMP');
    
    for await (const batch of parser.parseStream(stream)) {
      // Check memory usage
      if (this.currentMemoryUsage > this.maxMemoryMB * 0.8) {
        await this.waitForMemory();
      }
      
      const data = transformer.transformBatch(batch);
      await processor(data);
      
      // Force garbage collection if available
      if (global.gc) {
        global.gc();
      }
    }
  }

  private async waitForMemory(): Promise<void> {
    console.log('Waiting for memory to free up...');
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
}
```

### 3. Bandwidth Management

```typescript
export class BandwidthMonitor {
  private bytesDownloaded: number = 0;
  private startTime: number = Date.now();
  private measurements: { timestamp: number; bytes: number }[] = [];
  
  recordBytes(bytes: number): void {
    this.bytesDownloaded += bytes;
    this.measurements.push({
      timestamp: Date.now(),
      bytes
    });
    
    // Keep only last 60 seconds of measurements
    const cutoff = Date.now() - 60000;
    this.measurements = this.measurements.filter(m => m.timestamp > cutoff);
  }

  getCurrentBandwidthMbps(): number {
    if (this.measurements.length < 2) return 0;
    
    const recent = this.measurements.slice(-10);
    const timeSpan = recent[recent.length - 1].timestamp - recent[0].timestamp;
    const bytes = recent.reduce((sum, m) => sum + m.bytes, 0);
    
    return (bytes * 8) / (timeSpan * 1000); // Convert to Mbps
  }

  shouldThrottle(maxBandwidthMbps: number): boolean {
    return this.getCurrentBandwidthMbps() > maxBandwidthMbps * 0.9;
  }
}
```

## Integration with Existing System

### 1. Provider Interface Implementation

```typescript
import { BaseProvider } from '../BaseProvider';

export class PolygonS3Provider extends BaseProvider {
  private s3Client: PolygonS3Client;
  private scheduler: SmartDownloadScheduler;
  private storage: TimescaleDBStorage;
  
  constructor(config: PolygonProviderConfig) {
    super('polygon-s3');
    
    this.s3Client = new PolygonS3Client({
      accessKeyId: config.awsAccessKey,
      secretAccessKey: config.awsSecretKey,
      bucketName: config.bucketName || 'polygon-flat-files',
      region: config.region || 'us-east-1'
    });

    this.scheduler = new SmartDownloadScheduler(this.s3Client, {
      maxConcurrentDownloads: config.maxConcurrency || 10,
      maxBandwidthMbps: config.maxBandwidthMbps,
      prioritySymbols: config.prioritySymbols
    });

    this.storage = new TimescaleDBStorage(config.dbConfig);
  }

  async backfillHistoricalData(
    symbols: string[],
    startDate: Date,
    endDate: Date
  ): Promise<void> {
    // Set up event listeners
    this.scheduler.on('progress', (progress) => {
      this.emit('backfillProgress', progress);
    });

    this.scheduler.on('error', (error) => {
      this.emit('backfillError', error);
    });

    // Start download and processing
    await this.scheduler.scheduleDownloads(symbols, startDate, endDate);
  }

  async getHistoricalData(
    symbol: string,
    startDate: Date,
    endDate: Date
  ): Promise<TimeSeriesData[]> {
    // Check if data exists in storage
    const existingData = await this.storage.query({
      symbol,
      startDate,
      endDate
    });

    if (existingData.length > 0) {
      return existingData;
    }

    // Download if not available
    await this.backfillHistoricalData([symbol], startDate, endDate);
    
    // Return downloaded data
    return await this.storage.query({
      symbol,
      startDate,
      endDate
    });
  }
}
```

### 2. Usage Example

```typescript
// Initialize provider
const provider = new PolygonS3Provider({
  awsAccessKey: process.env.AWS_ACCESS_KEY,
  awsSecretKey: process.env.AWS_SECRET_KEY,
  maxConcurrency: 20,
  maxBandwidthMbps: 100,
  prioritySymbols: ['AAPL', 'GOOGL', 'MSFT'],
  dbConfig: {
    host: 'localhost',
    port: 5432,
    database: 'neural_trader',
    user: 'postgres',
    password: 'password'
  }
});

// Listen for progress
provider.on('backfillProgress', (progress) => {
  console.log(`Progress: ${progress.percentComplete}% complete`);
});

// Backfill data
await provider.backfillHistoricalData(
  ['AAPL', 'GOOGL', 'MSFT'],
  new Date('2020-01-01'),
  new Date('2023-12-31')
);
```

## Monitoring and Observability

### Metrics to Track

1. **Download Metrics**
   - Files downloaded per minute
   - Bytes downloaded per second
   - Average file size
   - Download success rate
   - Retry count per file

2. **Processing Metrics**
   - Records processed per second
   - Valid vs invalid records ratio
   - Parse errors per file
   - Memory usage
   - CPU utilization

3. **Storage Metrics**
   - Records inserted per second
   - Database write latency
   - Storage space used
   - Compression ratio achieved

### Logging Strategy

```typescript
import winston from 'winston';

const logger = winston.createLogger({
  level: 'info',
  format: winston.format.json(),
  transports: [
    new winston.transports.File({ filename: 'error.log', level: 'error' }),
    new winston.transports.File({ filename: 'combined.log' }),
    new winston.transports.Console({
      format: winston.format.simple()
    })
  ]
});

// Structured logging
logger.info('Download started', {
  job: job.id,
  symbol: job.symbol,
  date: job.date,
  s3Key: job.s3Key
});
```

## Security Considerations

1. **AWS Credentials**
   - Use IAM roles when running on AWS infrastructure
   - Store credentials in secure environment variables
   - Rotate access keys regularly
   - Use minimal required permissions

2. **S3 Access**
   - Use presigned URLs for temporary access
   - Enable S3 bucket versioning
   - Configure bucket policies for IP restrictions
   - Enable CloudTrail logging

3. **Data Validation**
   - Validate all incoming data
   - Sanitize data before storage
   - Check file checksums when available
   - Monitor for anomalous data patterns

## Testing Strategy

### Unit Tests

```typescript
describe('GzippedCSVParser', () => {
  it('should parse gzipped CSV correctly', async () => {
    const mockStream = createMockGzippedCSV([
      'timestamp,open,high,low,close,volume',
      '1640995200000,420.12,420.45,419.80,420.22,125432'
    ]);

    const parser = new GzippedCSVParser();
    const results = [];
    
    for await (const batch of parser.parseStream(mockStream)) {
      results.push(...batch);
    }

    expect(results).toHaveLength(1);
    expect(results[0].open).toBe(420.12);
  });
});
```

### Integration Tests

```typescript
describe('PolygonS3Provider Integration', () => {
  it('should download and process files correctly', async () => {
    const provider = new PolygonS3Provider(testConfig);
    
    const data = await provider.getHistoricalData(
      'AAPL',
      new Date('2023-01-01'),
      new Date('2023-01-02')
    );

    expect(data).toBeDefined();
    expect(data.length).toBeGreaterThan(0);
    expect(data[0].symbol).toBe('AAPL');
  });
});
```

## Conclusion

This S3 integration design provides a robust, scalable solution for downloading and processing Polygon's flat files. Key features include:

1. **Efficient Streaming**: Memory-efficient processing of large gzipped files
2. **Concurrent Downloads**: Parallel processing with configurable limits
3. **Smart Scheduling**: Priority-based downloading with bandwidth management
4. **Error Recovery**: Comprehensive error handling with retry strategies
5. **Performance Optimization**: Connection pooling, batching, and streaming
6. **Monitoring**: Detailed metrics and logging for observability

The implementation is designed to handle millions of records efficiently while maintaining data quality and system stability.