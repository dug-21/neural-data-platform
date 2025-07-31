/**
 * Example usage of the Polygon S3 Provider
 */

import { PolygonS3Provider, DataStorageAdapter, TimeSeriesData } from './index';

// Example storage adapter implementation
class InMemoryStorageAdapter implements DataStorageAdapter {
  private data: Map<string, TimeSeriesData[]> = new Map();

  async storeBatch(data: TimeSeriesData[]): Promise<void> {
    for (const item of data) {
      const key = `${item.symbol}_${item.timestamp.toISOString().split('T')[0]}`;
      const existing = this.data.get(key) || [];
      existing.push(item);
      this.data.set(key, existing);
    }
    console.log(`Stored ${data.length} records in memory`);
  }

  async queryData(params: {
    symbol: string;
    startDate: Date;
    endDate: Date;
  }): Promise<TimeSeriesData[]> {
    const results: TimeSeriesData[] = [];
    
    for (const [key, data] of this.data.entries()) {
      if (key.startsWith(params.symbol)) {
        for (const item of data) {
          if (item.timestamp >= params.startDate && item.timestamp <= params.endDate) {
            results.push(item);
          }
        }
      }
    }
    
    return results.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());
  }

  async checkDataExists(params: {
    symbol: string;
    date: Date;
  }): Promise<boolean> {
    const key = `${params.symbol}_${params.date.toISOString().split('T')[0]}`;
    return this.data.has(key);
  }
}

async function main() {
  // Initialize provider with configuration
  const provider = new PolygonS3Provider({
    // AWS credentials (can also use IAM role)
    awsAccessKeyId: process.env.AWS_ACCESS_KEY_ID,
    awsSecretAccessKey: process.env.AWS_SECRET_ACCESS_KEY,
    
    // Download configuration
    maxConcurrentDownloads: 20,
    maxBandwidthMbps: 100,  // Limit bandwidth to 100 Mbps
    
    // Processing configuration
    batchSize: 5000,
    validateData: true,
    retryAttempts: 3,
    
    // Priority symbols (downloaded first)
    prioritySymbols: ['SPY', 'QQQ', 'AAPL', 'MSFT', 'GOOGL'],
    
    // Download only during off-peak hours
    downloadTimeWindow: {
      startHour: 20,  // 8 PM
      endHour: 8      // 8 AM
    }
  });

  // Set up storage adapter
  const storage = new InMemoryStorageAdapter();
  provider.setStorageAdapter(storage);

  // Set up event handlers
  provider.on('connectionSuccess', () => {
    console.log('✅ Connected to S3 successfully');
  });

  provider.on('connectionError', (error) => {
    console.error('❌ Connection failed:', error);
  });

  provider.on('backfillStart', (options) => {
    console.log('\n🚀 Starting backfill:');
    console.log(`  Symbols: ${options.symbols.join(', ')}`);
    console.log(`  Date range: ${options.startDate.toDateString()} - ${options.endDate.toDateString()}`);
  });

  provider.on('backfillProgress', (progress) => {
    console.log(`\n📊Progress [${progress.phase}]:`);
    console.log(`  Days: ${progress.completedDays}/${progress.totalDays}`);
    console.log(`  Records: ${progress.totalRecords.toLocaleString()}`);
    console.log(`  Speed: ${progress.recordsPerSecond.toFixed(0)} records/sec`);
    console.log(`  ETA: ${Math.floor(progress.estimatedTimeRemaining / 60)} minutes`);
    
    if (progress.errors > 0) {
      console.log(`  ⚠️  Errors: ${progress.errors}`);
    }
  });

  provider.on('fileProcessed', ({ symbol, date, records }) => {
    console.log(`  ✅ ${symbol} ${date}: ${records.toLocaleString()} records`);
  });

  provider.on('fileFailed', ({ symbol, date, error }) => {
    console.error(`  ❌ ${symbol} ${date}: ${error}`);
  });

  provider.on('downloadProgress', (progress) => {
    const bar = '█'.repeat(Math.floor(progress.percentComplete / 5));
    const empty = '░'.repeat(20 - Math.floor(progress.percentComplete / 5));
    console.log(
      `  Download [${bar}${empty}] ${progress.percentComplete.toFixed(1)}% @ ${progress.throughputMbps?.toFixed(2)} Mbps`
    );
  });

  provider.on('backfillComplete', (result) => {
    console.log('\n✅ Backfill complete!');
    console.log(`  Total records: ${result.totalRecordsProcessed.toLocaleString()}`);
    console.log(`  Files downloaded: ${result.totalFilesDownloaded}`);
    console.log(`  Failed downloads: ${result.failedDownloads}`);
    console.log(`  Duration: ${Math.floor(result.duration / 60)} minutes`);
    
    if (result.errors.length > 0) {
      console.log('\n⚠️  Errors:');
      for (const error of result.errors) {
        console.log(`  - ${error.symbol} ${error.date}: ${error.error}`);
      }
    }
  });

  try {
    // Test connection first
    console.log('Testing S3 connection...');
    const connected = await provider.testConnection();
    if (!connected) {
      throw new Error('Failed to connect to S3');
    }

    // Example 1: List available files
    console.log('\n📂 Checking available files for AAPL...');
    const files = await provider.listAvailableFiles(
      'AAPL',
      new Date('2023-12-01'),
      new Date('2023-12-05')
    );
    console.log(`Found ${files.length} files:`);
    for (const file of files.slice(0, 3)) {
      console.log(`  - ${file.date}: ${(file.size / 1024 / 1024).toFixed(2)} MB`);
    }

    // Example 2: Backfill historical data for multiple symbols
    console.log('\n📥 Starting backfill...');
    const result = await provider.backfillHistoricalData({
      symbols: ['AAPL', 'GOOGL', 'MSFT'],
      startDate: new Date('2023-12-01'),
      endDate: new Date('2023-12-05'),
      assetClass: 'stocks',
      dataType: 'aggregates',
      timeframe: 'minute',
      overwriteExisting: false,
      onProgress: (progress) => {
        // Custom progress handler (optional)
        if (progress.phase === 'storing') {
          console.log('  💾 Storing data in database...');
        }
      }
    });

    // Example 3: Query stored data
    console.log('\n🔍 Querying stored data...');
    const data = await provider.getHistoricalData(
      'AAPL',
      new Date('2023-12-01'),
      new Date('2023-12-02')
    );
    console.log(`Retrieved ${data.length} data points`);
    
    if (data.length > 0) {
      console.log('\nSample data:');
      const sample = data.slice(0, 5);
      for (const item of sample) {
        console.log(`  ${item.timestamp.toISOString()}: O=${item.open.toFixed(2)} H=${item.high.toFixed(2)} L=${item.low.toFixed(2)} C=${item.close.toFixed(2)} V=${item.volume.toLocaleString()}`);
      }
    }

    // Example 4: Pause/Resume/Cancel operations
    console.log('\n🎯 Advanced operations example:');
    
    // Start a large backfill
    const largeBackfillPromise = provider.backfillHistoricalData({
      symbols: ['SPY', 'QQQ', 'IWM', 'DIA', 'VTI'],
      startDate: new Date('2023-01-01'),
      endDate: new Date('2023-12-31'),
      assetClass: 'stocks',
      dataType: 'aggregates',
      timeframe: 'minute'
    });

    // Simulate pause after 5 seconds
    setTimeout(() => {
      console.log('\n⏸  Pausing backfill...');
      provider.pauseBackfill();
      
      // Resume after 3 seconds
      setTimeout(() => {
        console.log('▶️  Resuming backfill...');
        provider.resumeBackfill();
      }, 3000);
    }, 5000);

    // Check status periodically
    const statusInterval = setInterval(() => {
      const status = provider.getBackfillStatus();
      if (status.isRunning && status.progress) {
        console.log(`Status: ${status.progress.completedDays}/${status.progress.totalDays} days completed`);
      }
    }, 10000);

    // Wait for completion or timeout
    await Promise.race([
      largeBackfillPromise,
      new Promise((resolve) => setTimeout(resolve, 30000)) // 30 second timeout for demo
    ]);

    clearInterval(statusInterval);

    // Cancel if still running (for demo purposes)
    const finalStatus = provider.getBackfillStatus();
    if (finalStatus.isRunning) {
      console.log('\n🚫 Cancelling remaining downloads...');
      await provider.cancelBackfill();
    }

  } catch (error) {
    console.error('\n❌ Error:', error.message);
    process.exit(1);
  }
}

// Run the example
if (require.main === module) {
  main().catch(console.error);
}