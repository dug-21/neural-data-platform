// Export main provider
export { PolygonS3Provider } from './polygonS3Provider';
export type { 
  PolygonS3Config, 
  DataStorageAdapter, 
  BackfillOptions, 
  BackfillProgress, 
  BackfillResult 
} from './polygonS3Provider';

// Export S3 client
export { PolygonS3Client } from './s3Client';
export type { S3Config, S3FileInfo, S3DownloadProgress } from './s3Client';

// Export CSV parser
export { GzippedCSVParser } from './csvParser';
export type { MarketDataRow, TradeDataRow, ParseOptions } from './csvParser';

// Export data transformer
export { PolygonDataTransformer } from './dataTransformer';
export type { TimeSeriesData, TransformOptions, ValidationResult } from './dataTransformer';

// Export download manager
export { ConcurrentDownloadManager } from './downloadManager';
export type { 
  DownloadJob, 
  DownloadProgress, 
  DownloadResult, 
  DownloadManagerConfig, 
  DownloadStatistics 
} from './downloadManager';

// Export scheduler
export { SmartDownloadScheduler } from './downloadScheduler';
export type { 
  SchedulerConfig, 
  ScheduleOptions, 
  SchedulerProgress, 
  SchedulerResult 
} from './downloadScheduler';