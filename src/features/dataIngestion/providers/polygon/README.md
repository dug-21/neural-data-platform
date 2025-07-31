# Polygon S3 Flat File Integration

This module provides a comprehensive solution for downloading, parsing, and processing Polygon's historical market data from their S3 flat file storage.

## Features

- 🚀 **High-Performance Streaming**: Memory-efficient processing of large gzipped CSV files
- ⚡ **Concurrent Downloads**: Parallel downloading with configurable concurrency limits
- 🔄 **Smart Scheduling**: Priority-based downloading with bandwidth management
- 🛡️ **Robust Error Handling**: Automatic retries with exponential backoff
- 📊 **Progress Tracking**: Real-time progress updates and throughput monitoring
- 💾 **Storage Integration**: Pluggable storage adapter interface
- 🔧 **Flexible Configuration**: Extensive configuration options for different use cases

## Installation

```bash
npm install @aws-sdk/client-s3 @aws-sdk/s3-request-presigner csv-parse luxon p-limit exponential-backoff
```

## Quick Start

```typescript
import { PolygonS3Provider } from './providers/polygon';

// Initialize provider
const provider = new PolygonS3Provider({
  awsAccessKeyId: process.env.AWS_ACCESS_KEY_ID,
  awsSecretAccessKey: process.env.AWS_SECRET_ACCESS_KEY,
  maxConcurrentDownloads: 20,
  prioritySymbols: ['AAPL', 'GOOGL', 'MSFT']
});

// Test connection
const connected = await provider.testConnection();
if (!connected) {
  throw new Error('Failed to connect to S3');
}

// Backfill historical data
const result = await provider.backfillHistoricalData({
  symbols: ['AAPL', 'GOOGL', 'MSFT'],
  startDate: new Date('2023-01-01'),
  endDate: new Date('2023-12-31'),
  assetClass: 'stocks',
  dataType: 'aggregates',
  timeframe: 'minute',
  onProgress: (progress) => {
    console.log(`Progress: ${progress.completedDays}/${progress.totalDays} days`);
    console.log(`Records/sec: ${progress.recordsPerSecond.toFixed(0)}`);
  }
});

console.log(`Backfill complete: ${result.totalRecordsProcessed} records processed`);
```

## Configuration Options

### S3 Configuration

```typescript
interface S3Config {
  awsAccessKeyId?: string;      // AWS access key (or use IAM role)
  awsSecretAccessKey?: string;  // AWS secret key
  bucketName?: string;          // Default: 'polygon-flat-files'
  region?: string;              // Default: 'us-east-1'
}
```

### Download Configuration

```typescript
interface DownloadConfig {
  maxConcurrentDownloads?: number;  // Default: 10
  maxBandwidthMbps?: number;        // Bandwidth limit (default: unlimited)
  downloadTimeWindow?: {            // Time window for downloads
    startHour: number;              // 0-23 (default: 0)
    endHour: number;                // 0-23 (default: 24)
  };
  batchSize?: number;               // Records per batch (default: 5000)
  validateData?: boolean;           // Validate OHLCV data (default: true)
  retryAttempts?: number;           // Max retry attempts (default: 3)
}
```

### Priority Configuration

```typescript
interface PriorityConfig {
  prioritySymbols?: string[];  // Symbols to download first
}
```

## Storage Integration

Implement the `DataStorageAdapter` interface to integrate with your storage system:

```typescript
interface DataStorageAdapter {
  storeBatch(data: TimeSeriesData[]): Promise<void>;
  queryData(params: {
    symbol: string;
    startDate: Date;
    endDate: Date;
  }): Promise<TimeSeriesData[]>;
  checkDataExists(params: {
    symbol: string;
    date: Date;
  }): Promise<boolean>;
}

// Example: TimescaleDB adapter
class TimescaleDBAdapter implements DataStorageAdapter {
  async storeBatch(data: TimeSeriesData[]): Promise<void> {
    // Bulk insert into TimescaleDB
    await db.insertMany('market_data', data);
  }

  async queryData(params): Promise<TimeSeriesData[]> {
    return await db.query(
      'SELECT * FROM market_data WHERE symbol = $1 AND time BETWEEN $2 AND $3',
      [params.symbol, params.startDate, params.endDate]
    );
  }

  async checkDataExists(params): Promise<boolean> {
    const count = await db.count(
      'SELECT COUNT(*) FROM market_data WHERE symbol = $1 AND DATE(time) = $2',
      [params.symbol, params.date]
    );
    return count > 0;
  }
}

// Use with provider
const storageAdapter = new TimescaleDBAdapter();
provider.setStorageAdapter(storageAdapter);
```

## Event Handling

The provider emits various events for monitoring:

```typescript
// Connection events
provider.on('connectionSuccess', (message) => {
  console.log('Connected to S3:', message);
});

provider.on('connectionError', (error) => {
  console.error('Connection failed:', error);
});

// Backfill events
provider.on('backfillStart', (options) => {
  console.log('Backfill started:', options);
});

provider.on('backfillProgress', (progress) => {
  console.log(`Progress: ${progress.phase} - ${progress.completedDays}/${progress.totalDays}`);
});

provider.on('backfillComplete', (result) => {
  console.log('Backfill complete:', result);
});

// File processing events
provider.on('fileProcessed', ({ symbol, date, records }) => {
  console.log(`Processed ${symbol} for ${date}: ${records} records`);
});

provider.on('fileFailed', ({ symbol, date, error }) => {
  console.error(`Failed to process ${symbol} for ${date}:`, error);
});

// Download progress
provider.on('downloadProgress', (progress) => {
  console.log(`Download: ${progress.percentComplete.toFixed(1)}% @ ${progress.throughputMbps.toFixed(2)} Mbps`);
});
```

## Advanced Usage

### Bandwidth Management

```typescript
const provider = new PolygonS3Provider({
  maxBandwidthMbps: 100,  // Limit to 100 Mbps
  downloadTimeWindow: {
    startHour: 22,  // Download only between 10 PM
    endHour: 6      // and 6 AM
  }
});
```

### Custom Data Validation

```typescript
// The provider validates:
// - Positive prices
// - OHLC relationships (high >= low, etc.)
// - Volume >= 0
// - Extreme price movements (>50% in a minute)
// - Valid timestamps

// Disable validation for faster processing
const provider = new PolygonS3Provider({
  validateData: false
});
```

### Pause/Resume/Cancel Operations

```typescript
// Start backfill
const backfillPromise = provider.backfillHistoricalData(options);

// Pause
provider.pauseBackfill();

// Resume
provider.resumeBackfill();

// Cancel
await provider.cancelBackfill();

// Check status
const status = provider.getBackfillStatus();
console.log('Is running:', status.isRunning);
console.log('Progress:', status.progress);
```

### List Available Files

```typescript
const files = await provider.listAvailableFiles(
  'AAPL',
  new Date('2023-01-01'),
  new Date('2023-01-31')
);

console.log('Available files:', files);
// Output: [{ date: '2023-01-01', s3Key: 'stocks/2023/01/01/...', size: 1234567 }, ...]
```

## Performance Optimization

### 1. Concurrent Downloads

```typescript
// Increase concurrency for faster downloads
const provider = new PolygonS3Provider({
  maxConcurrentDownloads: 50  // Be careful not to hit S3 rate limits
});
```

### 2. Batch Size

```typescript
// Larger batches = fewer DB writes but more memory usage
const provider = new PolygonS3Provider({
  batchSize: 10000  // Process 10k records at a time
});
```

### 3. Priority Symbols

```typescript
// Download important symbols first
const provider = new PolygonS3Provider({
  prioritySymbols: ['SPY', 'QQQ', 'AAPL', 'MSFT', 'GOOGL']
});
```

## Error Handling

The provider includes comprehensive error handling:

- **Network Errors**: Automatic retry with exponential backoff
- **S3 Access Errors**: Proper error messages for permissions issues
- **Parse Errors**: Skip invalid rows or fail based on configuration
- **Validation Errors**: Detailed validation error reporting
- **Storage Errors**: Graceful handling of database failures

```typescript
try {
  await provider.backfillHistoricalData(options);
} catch (error) {
  if (error.code === 'AccessDenied') {
    console.error('S3 access denied. Check your credentials.');
  } else if (error.code === 'NoSuchKey') {
    console.error('File not found in S3.');
  } else {
    console.error('Backfill failed:', error);
  }
}
```

## Monitoring and Metrics

Key metrics to monitor:

1. **Download Metrics**
   - Files downloaded per minute
   - Bytes downloaded per second
   - Download success rate
   - Retry count per file

2. **Processing Metrics**
   - Records processed per second
   - Valid vs invalid records ratio
   - Parse errors per file
   - Memory usage

3. **Storage Metrics**
   - Records inserted per second
   - Database write latency
   - Storage space used

## Troubleshooting

### Common Issues

1. **S3 Access Denied**
   - Check AWS credentials
   - Verify bucket permissions
   - Ensure correct region

2. **Slow Downloads**
   - Increase concurrent downloads
   - Check bandwidth limits
   - Verify network connectivity

3. **Memory Issues**
   - Reduce batch size
   - Process fewer symbols at once
   - Enable Node.js garbage collection

4. **Parse Errors**
   - Check CSV format changes
   - Enable skip invalid rows
   - Review error logs

## Best Practices

1. **Start Small**: Test with one symbol and one day before large backfills
2. **Monitor Progress**: Use event handlers to track progress
3. **Handle Failures**: Implement proper error handling and recovery
4. **Optimize Batching**: Balance batch size with memory usage
5. **Use Priority**: Download most important data first
6. **Schedule Wisely**: Use off-peak hours for large downloads
7. **Validate Data**: Keep validation enabled for data quality

## License

This module is part of the neural-trader project.