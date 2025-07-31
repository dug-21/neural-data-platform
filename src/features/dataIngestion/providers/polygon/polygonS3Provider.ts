import { EventEmitter } from 'events';
import { PolygonS3Client, S3Config } from './s3Client';
import { SmartDownloadScheduler, ScheduleOptions, SchedulerResult } from './downloadScheduler';
import { TimeSeriesData } from './dataTransformer';
import { DownloadJob } from './downloadManager';

export interface PolygonS3Config {
  // S3 Configuration
  awsAccessKeyId?: string;
  awsSecretAccessKey?: string;
  bucketName?: string;
  region?: string;
  
  // Download Configuration
  maxConcurrentDownloads?: number;
  maxBandwidthMbps?: number;
  downloadTimeWindow?: {
    startHour: number;
    endHour: number;
  };
  
  // Processing Configuration
  batchSize?: number;
  validateData?: boolean;
  retryAttempts?: number;
  
  // Priority Configuration
  prioritySymbols?: string[];
  
  // Storage Configuration
  storageAdapter?: DataStorageAdapter;
}

export interface DataStorageAdapter {
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

export interface BackfillOptions {
  symbols: string[];
  startDate: Date;
  endDate: Date;
  assetClass?: 'stocks' | 'options' | 'crypto' | 'forex';
  dataType?: 'aggregates' | 'trades';
  timeframe?: 'minute' | 'hour' | 'day';
  overwriteExisting?: boolean;
  onProgress?: (progress: BackfillProgress) => void;
}

export interface BackfillProgress {
  phase: 'preparing' | 'downloading' | 'processing' | 'storing' | 'complete';
  totalSymbols: number;
  completedSymbols: number;
  totalDays: number;
  completedDays: number;
  totalRecords: number;
  recordsPerSecond: number;
  estimatedTimeRemaining: number; // seconds
  errors: number;
}

export interface BackfillResult {
  success: boolean;
  totalRecordsProcessed: number;
  totalFilesDownloaded: number;
  failedDownloads: number;
  duration: number; // seconds
  errors: Array<{
    symbol: string;
    date: string;
    error: string;
  }>;
}

/**
 * Main provider class for Polygon S3 flat file integration
 */
export class PolygonS3Provider extends EventEmitter {
  private s3Client: PolygonS3Client;
  private scheduler: SmartDownloadScheduler;
  private config: Required<PolygonS3Config>;
  private storageAdapter?: DataStorageAdapter;
  
  // State tracking
  private isBackfilling: boolean = false;
  private currentBackfillOptions?: BackfillOptions;
  private backfillStartTime?: Date;
  private processedRecords: number = 0;
  private errors: Array<{ symbol: string; date: string; error: string }> = [];

  constructor(config: PolygonS3Config = {}) {
    super();
    
    // Set default configuration
    this.config = {
      // S3 defaults
      awsAccessKeyId: config.awsAccessKeyId || process.env.AWS_ACCESS_KEY_ID,
      awsSecretAccessKey: config.awsSecretAccessKey || process.env.AWS_SECRET_ACCESS_KEY,
      bucketName: config.bucketName || 'polygon-flat-files',
      region: config.region || 'us-east-1',
      
      // Download defaults
      maxConcurrentDownloads: config.maxConcurrentDownloads || 10,
      maxBandwidthMbps: config.maxBandwidthMbps || Infinity,
      downloadTimeWindow: config.downloadTimeWindow || { startHour: 0, endHour: 24 },
      
      // Processing defaults
      batchSize: config.batchSize || 5000,
      validateData: config.validateData !== false,
      retryAttempts: config.retryAttempts || 3,
      
      // Priority defaults
      prioritySymbols: config.prioritySymbols || [],
      
      // Storage
      storageAdapter: config.storageAdapter
    };
    
    this.storageAdapter = this.config.storageAdapter;
    
    // Initialize S3 client
    this.s3Client = new PolygonS3Client({
      accessKeyId: this.config.awsAccessKeyId,
      secretAccessKey: this.config.awsSecretAccessKey,
      bucketName: this.config.bucketName,
      region: this.config.region,
      maxRetries: this.config.retryAttempts
    });
    
    // Initialize scheduler
    this.scheduler = new SmartDownloadScheduler(this.s3Client, {
      maxConcurrentDownloads: this.config.maxConcurrentDownloads,
      maxBandwidthMbps: this.config.maxBandwidthMbps,
      prioritySymbols: this.config.prioritySymbols,
      downloadTimeWindow: this.config.downloadTimeWindow,
      batchSize: Math.floor(this.config.maxConcurrentDownloads * 1.5)
    });
    
    // Set up event handlers
    this.setupEventHandlers();
  }

  /**
   * Test S3 connection
   */
  async testConnection(): Promise<boolean> {
    try {
      const connected = await this.s3Client.testConnection();
      
      if (connected) {
        this.emit('connectionSuccess', 'S3 connection successful');
      } else {
        this.emit('connectionError', 'S3 connection failed');
      }
      
      return connected;
    } catch (error) {
      this.emit('connectionError', error);
      return false;
    }
  }

  /**
   * Backfill historical data for multiple symbols
   */
  async backfillHistoricalData(options: BackfillOptions): Promise<BackfillResult> {
    if (this.isBackfilling) {
      throw new Error('Backfill already in progress');
    }

    this.isBackfilling = true;
    this.currentBackfillOptions = options;
    this.backfillStartTime = new Date();
    this.processedRecords = 0;
    this.errors = [];

    try {
      // Emit start event
      this.emit('backfillStart', options);
      this.updateProgress('preparing');

      // Check existing data if not overwriting
      let scheduleOptions: ScheduleOptions = {
        symbols: options.symbols,
        startDate: options.startDate,
        endDate: options.endDate,
        assetClass: options.assetClass,
        dataType: options.dataType,
        timeframe: options.timeframe,
        overwriteExisting: options.overwriteExisting
      };

      if (!options.overwriteExisting && this.storageAdapter) {
        scheduleOptions = await this.filterExistingData(scheduleOptions);
      }

      // Schedule downloads
      this.updateProgress('downloading');
      
      const result = await this.scheduler.scheduleDownloads(scheduleOptions);
      
      // Process results
      const backfillResult: BackfillResult = {
        success: result.failedJobs === 0,
        totalRecordsProcessed: result.totalRecords,
        totalFilesDownloaded: result.successfulJobs,
        failedDownloads: result.failedJobs,
        duration: result.totalDuration,
        errors: result.failedJobDetails.map(detail => ({
          symbol: detail.job.symbol,
          date: detail.job.date,
          error: detail.error.message
        }))
      };

      // Emit completion
      this.updateProgress('complete');
      this.emit('backfillComplete', backfillResult);
      
      return backfillResult;
    } catch (error) {
      const errorResult: BackfillResult = {
        success: false,
        totalRecordsProcessed: this.processedRecords,
        totalFilesDownloaded: 0,
        failedDownloads: 0,
        duration: this.backfillStartTime 
          ? (Date.now() - this.backfillStartTime.getTime()) / 1000 
          : 0,
        errors: [{
          symbol: 'all',
          date: 'all',
          error: error.message
        }]
      };
      
      this.emit('backfillError', error);
      return errorResult;
    } finally {
      this.isBackfilling = false;
      this.currentBackfillOptions = undefined;
    }
  }

  /**
   * Get historical data for a single symbol
   */
  async getHistoricalData(
    symbol: string,
    startDate: Date,
    endDate: Date,
    options: {
      assetClass?: 'stocks' | 'options' | 'crypto' | 'forex';
      dataType?: 'aggregates' | 'trades';
      timeframe?: 'minute' | 'hour' | 'day';
    } = {}
  ): Promise<TimeSeriesData[]> {
    // First check if data exists in storage
    if (this.storageAdapter) {
      try {
        const existingData = await this.storageAdapter.queryData({
          symbol,
          startDate,
          endDate
        });
        
        if (existingData.length > 0) {
          return existingData;
        }
      } catch (error) {
        console.warn('Error querying existing data:', error);
      }
    }

    // If not, download it
    const backfillResult = await this.backfillHistoricalData({
      symbols: [symbol],
      startDate,
      endDate,
      assetClass: options.assetClass,
      dataType: options.dataType,
      timeframe: options.timeframe,
      overwriteExisting: false
    });

    if (!backfillResult.success) {
      throw new Error(`Failed to download data: ${backfillResult.errors[0]?.error}`);
    }

    // Query again after download
    if (this.storageAdapter) {
      return await this.storageAdapter.queryData({
        symbol,
        startDate,
        endDate
      });
    }

    return [];
  }

  /**
   * List available files for a symbol and date range
   */
  async listAvailableFiles(
    symbol: string,
    startDate: Date,
    endDate: Date,
    options: {
      assetClass?: 'stocks' | 'options' | 'crypto' | 'forex';
      dataType?: 'aggregates' | 'trades';
    } = {}
  ): Promise<Array<{ date: string; s3Key: string; size: number }>> {
    const files: Array<{ date: string; s3Key: string; size: number }> = [];
    const dates = this.getDateRange(startDate, endDate);
    const assetClass = options.assetClass || 'stocks';
    const dataType = options.dataType || 'aggregates';

    for (const date of dates) {
      const prefix = this.buildS3Prefix({
        assetClass,
        dataType,
        date
      });

      try {
        const s3Files = await this.s3Client.listFiles(prefix);
        
        for (const file of s3Files) {
          if (file.key.includes(symbol)) {
            files.push({
              date: this.formatDate(date),
              s3Key: file.key,
              size: file.size
            });
          }
        }
      } catch (error) {
        console.warn(`Error listing files for ${symbol} on ${date}:`, error);
      }
    }

    return files;
  }

  /**
   * Pause ongoing backfill
   */
  pauseBackfill(): void {
    if (this.isBackfilling) {
      this.scheduler.pause();
      this.emit('backfillPaused');
    }
  }

  /**
   * Resume paused backfill
   */
  resumeBackfill(): void {
    if (this.isBackfilling) {
      this.scheduler.resume();
      this.emit('backfillResumed');
    }
  }

  /**
   * Cancel ongoing backfill
   */
  async cancelBackfill(): Promise<void> {
    if (this.isBackfilling) {
      await this.scheduler.cancel();
      this.isBackfilling = false;
      this.emit('backfillCancelled');
    }
  }

  /**
   * Get current backfill status
   */
  getBackfillStatus(): {
    isRunning: boolean;
    options?: BackfillOptions;
    progress?: BackfillProgress;
    schedulerStatus?: ReturnType<SmartDownloadScheduler['getStatus']>;
  } {
    if (!this.isBackfilling) {
      return { isRunning: false };
    }

    const schedulerStatus = this.scheduler.getStatus();
    const progress = this.calculateProgress();

    return {
      isRunning: true,
      options: this.currentBackfillOptions,
      progress,
      schedulerStatus
    };
  }

  /**
   * Set storage adapter
   */
  setStorageAdapter(adapter: DataStorageAdapter): void {
    this.storageAdapter = adapter;
  }

  /**
   * Update configuration
   */
  updateConfig(config: Partial<PolygonS3Config>): void {
    Object.assign(this.config, config);
    
    // Update scheduler config
    this.scheduler.updateConfig({
      maxConcurrentDownloads: config.maxConcurrentDownloads,
      maxBandwidthMbps: config.maxBandwidthMbps,
      prioritySymbols: config.prioritySymbols,
      downloadTimeWindow: config.downloadTimeWindow
    });
  }

  /**
   * Set up event handlers
   */
  private setupEventHandlers(): void {
    // Handle scheduler events
    this.scheduler.on('progress', (progress) => {
      this.updateProgress('downloading', progress);
    });

    this.scheduler.on('jobComplete', async (job: DownloadJob, result: any) => {
      this.processedRecords += result.recordsProcessed;
      
      // Store data if adapter is available
      if (this.storageAdapter && result.dataPoints) {
        try {
          this.updateProgress('storing');
          await this.storageAdapter.storeBatch(result.dataPoints);
        } catch (error) {
          console.error(`Error storing data for ${job.id}:`, error);
          this.errors.push({
            symbol: job.symbol,
            date: job.date,
            error: error.message
          });
        }
      }
      
      this.emit('fileProcessed', {
        symbol: job.symbol,
        date: job.date,
        records: result.recordsProcessed
      });
    });

    this.scheduler.on('jobFailed', (job: DownloadJob, error: Error) => {
      this.errors.push({
        symbol: job.symbol,
        date: job.date,
        error: error.message
      });
      
      this.emit('fileFailed', {
        symbol: job.symbol,
        date: job.date,
        error: error.message
      });
    });

    this.scheduler.on('downloadProgress', (progress) => {
      this.emit('downloadProgress', progress);
    });
  }

  /**
   * Filter out existing data
   */
  private async filterExistingData(options: ScheduleOptions): Promise<ScheduleOptions> {
    if (!this.storageAdapter) {
      return options;
    }

    const filteredSymbols: string[] = [];
    const dates = this.getDateRange(options.startDate, options.endDate);

    for (const symbol of options.symbols) {
      let hasAllData = true;
      
      for (const date of dates) {
        const exists = await this.storageAdapter.checkDataExists({
          symbol,
          date
        });
        
        if (!exists) {
          hasAllData = false;
          break;
        }
      }
      
      if (!hasAllData) {
        filteredSymbols.push(symbol);
      }
    }

    return {
      ...options,
      symbols: filteredSymbols
    };
  }

  /**
   * Update and emit progress
   */
  private updateProgress(phase: BackfillProgress['phase'], schedulerProgress?: any): void {
    const progress = this.calculateProgress(phase, schedulerProgress);
    
    if (this.currentBackfillOptions?.onProgress) {
      this.currentBackfillOptions.onProgress(progress);
    }
    
    this.emit('backfillProgress', progress);
  }

  /**
   * Calculate current progress
   */
  private calculateProgress(
    phase?: BackfillProgress['phase'],
    schedulerProgress?: any
  ): BackfillProgress {
    const options = this.currentBackfillOptions;
    if (!options) {
      return {
        phase: 'preparing',
        totalSymbols: 0,
        completedSymbols: 0,
        totalDays: 0,
        completedDays: 0,
        totalRecords: 0,
        recordsPerSecond: 0,
        estimatedTimeRemaining: 0,
        errors: 0
      };
    }

    const totalDays = this.getTotalDays(options.startDate, options.endDate);
    const elapsedTime = this.backfillStartTime
      ? (Date.now() - this.backfillStartTime.getTime()) / 1000
      : 0;

    return {
      phase: phase || 'downloading',
      totalSymbols: options.symbols.length,
      completedSymbols: schedulerProgress?.completedJobs 
        ? Math.floor(schedulerProgress.completedJobs / totalDays) 
        : 0,
      totalDays: totalDays * options.symbols.length,
      completedDays: schedulerProgress?.completedJobs || 0,
      totalRecords: this.processedRecords,
      recordsPerSecond: elapsedTime > 0 ? this.processedRecords / elapsedTime : 0,
      estimatedTimeRemaining: schedulerProgress?.estimatedTimeRemaining || 0,
      errors: this.errors.length
    };
  }

  /**
   * Get date range
   */
  private getDateRange(startDate: Date, endDate: Date): Date[] {
    const dates: Date[] = [];
    const current = new Date(startDate);
    current.setHours(0, 0, 0, 0);
    
    const end = new Date(endDate);
    end.setHours(0, 0, 0, 0);

    while (current <= end) {
      dates.push(new Date(current));
      current.setDate(current.getDate() + 1);
    }

    return dates;
  }

  /**
   * Get total days in range
   */
  private getTotalDays(startDate: Date, endDate: Date): number {
    const start = new Date(startDate).setHours(0, 0, 0, 0);
    const end = new Date(endDate).setHours(0, 0, 0, 0);
    return Math.ceil((end - start) / (1000 * 60 * 60 * 24)) + 1;
  }

  /**
   * Format date
   */
  private formatDate(date: Date): string {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    return `${year}-${month}-${day}`;
  }

  /**
   * Build S3 prefix
   */
  private buildS3Prefix(params: {
    assetClass: string;
    dataType: string;
    date: Date;
  }): string {
    const year = params.date.getFullYear();
    const month = String(params.date.getMonth() + 1).padStart(2, '0');
    const day = String(params.date.getDate()).padStart(2, '0');
    
    return `${params.assetClass}/${year}/${month}/${day}/`;
  }
}