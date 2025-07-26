import { EventEmitter } from 'events';
import { PolygonS3Client } from './s3Client';
import { ConcurrentDownloadManager, DownloadJob, DownloadResult } from './downloadManager';
import { DateTime } from 'luxon';

export interface SchedulerConfig {
  maxConcurrentDownloads?: number;
  maxBandwidthMbps?: number;
  prioritySymbols?: string[];
  downloadTimeWindow?: {
    startHour: number; // 0-23
    endHour: number;   // 0-23
  };
  batchSize?: number;
  pauseBetweenBatchesMs?: number;
}

export interface ScheduleOptions {
  symbols: string[];
  startDate: Date;
  endDate: Date;
  assetClass?: 'stocks' | 'options' | 'crypto' | 'forex';
  dataType?: 'aggregates' | 'trades';
  timeframe?: 'minute' | 'hour' | 'day';
  overwriteExisting?: boolean;
}

export interface SchedulerProgress {
  totalJobs: number;
  completedJobs: number;
  failedJobs: number;
  currentBatch: number;
  totalBatches: number;
  estimatedTimeRemaining: number; // seconds
  currentThroughputMbps: number;
}

export interface SchedulerResult {
  totalJobs: number;
  successfulJobs: number;
  failedJobs: number;
  totalRecords: number;
  totalDuration: number; // seconds
  averageThroughputMbps: number;
  failedJobDetails: Array<{
    job: DownloadJob;
    error: Error;
  }>;
}

/**
 * Smart scheduler for optimizing S3 downloads based on various factors
 */
export class SmartDownloadScheduler extends EventEmitter {
  private s3Client: PolygonS3Client;
  private downloader: ConcurrentDownloadManager;
  private config: Required<SchedulerConfig>;
  
  // Scheduling state
  private isRunning: boolean = false;
  private isPaused: boolean = false;
  private currentBatch: number = 0;
  private totalBatches: number = 0;
  private startTime: Date | null = null;
  
  // Performance tracking
  private completedJobs: number = 0;
  private failedJobs: number = 0;
  private totalRecords: number = 0;
  private bandwidthHistory: Array<{ timestamp: number; mbps: number }> = [];

  constructor(s3Client: PolygonS3Client, config: SchedulerConfig = {}) {
    super();
    this.s3Client = s3Client;
    
    // Set default configuration
    this.config = {
      maxConcurrentDownloads: config.maxConcurrentDownloads || 10,
      maxBandwidthMbps: config.maxBandwidthMbps || Infinity,
      prioritySymbols: config.prioritySymbols || [],
      downloadTimeWindow: config.downloadTimeWindow || { startHour: 0, endHour: 24 },
      batchSize: config.batchSize || 50,
      pauseBetweenBatchesMs: config.pauseBetweenBatchesMs || 1000
    };
    
    this.downloader = new ConcurrentDownloadManager(s3Client, {
      maxConcurrency: this.config.maxConcurrentDownloads
    });
    
    // Set up event forwarding
    this.setupEventHandlers();
  }

  /**
   * Schedule downloads for multiple symbols and date ranges
   */
  async scheduleDownloads(options: ScheduleOptions): Promise<SchedulerResult> {
    if (this.isRunning) {
      throw new Error('Scheduler is already running');
    }

    this.isRunning = true;
    this.startTime = new Date();
    this.completedJobs = 0;
    this.failedJobs = 0;
    this.totalRecords = 0;
    
    try {
      // Generate all download jobs
      const jobs = await this.generateDownloadJobs(options);
      
      if (jobs.length === 0) {
        throw new Error('No download jobs generated');
      }

      this.emit('scheduleStart', {
        totalJobs: jobs.length,
        symbols: options.symbols,
        dateRange: { start: options.startDate, end: options.endDate }
      });

      // Prioritize and batch jobs
      const prioritizedJobs = this.prioritizeJobs(jobs);
      const batches = this.createBatches(prioritizedJobs);
      this.totalBatches = batches.length;

      // Process batches
      const failedJobDetails: Array<{ job: DownloadJob; error: Error }> = [];
      
      for (let i = 0; i < batches.length; i++) {
        this.currentBatch = i + 1;
        
        // Check if within download window
        await this.waitForDownloadWindow();
        
        // Check bandwidth and adjust concurrency
        await this.adjustConcurrencyForBandwidth();
        
        // Check if paused
        while (this.isPaused) {
          await this.sleep(1000);
        }
        
        // Download batch
        const batchResults = await this.downloader.downloadBatch(batches[i]);
        
        // Process results
        for (const [jobId, result] of batchResults.entries()) {
          if (result.success) {
            this.completedJobs++;
            this.totalRecords += result.recordsProcessed;
          } else {
            this.failedJobs++;
            const job = batches[i].find(j => j.id === jobId);
            if (job && result.error) {
              failedJobDetails.push({ job, error: result.error });
            }
          }
        }
        
        // Emit progress
        this.emitProgress();
        
        // Pause between batches
        if (i < batches.length - 1) {
          await this.sleep(this.config.pauseBetweenBatchesMs);
        }
      }

      // Calculate final results
      const duration = (Date.now() - this.startTime.getTime()) / 1000;
      const stats = this.downloader.getStatistics();
      
      const result: SchedulerResult = {
        totalJobs: jobs.length,
        successfulJobs: this.completedJobs,
        failedJobs: this.failedJobs,
        totalRecords: this.totalRecords,
        totalDuration: duration,
        averageThroughputMbps: stats.averageThroughputMbps,
        failedJobDetails
      };

      this.emit('scheduleComplete', result);
      
      return result;
    } finally {
      this.isRunning = false;
      this.downloader.reset();
    }
  }

  /**
   * Generate download jobs for the specified parameters
   */
  private async generateDownloadJobs(options: ScheduleOptions): Promise<DownloadJob[]> {
    const jobs: DownloadJob[] = [];
    const { symbols, startDate, endDate, assetClass = 'stocks', dataType = 'aggregates', timeframe = 'minute' } = options;

    // Generate date range
    const dates = this.getDateRange(startDate, endDate);
    
    for (const symbol of symbols) {
      for (const date of dates) {
        // Skip weekends for stock market data
        if (assetClass === 'stocks' && this.isWeekend(date)) {
          continue;
        }

        const s3Key = PolygonS3Client.buildS3Key({
          assetClass,
          dataType,
          timeframe: dataType === 'aggregates' ? timeframe : undefined,
          symbol,
          date
        });

        const jobId = `${symbol}_${this.formatDate(date)}_${dataType}`;
        const priority = this.calculateJobPriority(symbol, date);

        jobs.push({
          id: jobId,
          symbol,
          date: this.formatDate(date),
          s3Key,
          priority,
          retries: 0,
          status: 'pending'
        });
      }
    }

    // Check which files exist (optional optimization)
    if (options.overwriteExisting === false) {
      // This would require checking existing data in storage
      // For now, we'll download all requested files
    }

    return jobs;
  }

  /**
   * Generate date range
   */
  private getDateRange(startDate: Date, endDate: Date): Date[] {
    const dates: Date[] = [];
    const current = DateTime.fromJSDate(startDate).startOf('day');
    const end = DateTime.fromJSDate(endDate).startOf('day');

    let cursor = current;
    while (cursor <= end) {
      dates.push(cursor.toJSDate());
      cursor = cursor.plus({ days: 1 });
    }

    return dates;
  }

  /**
   * Check if date is weekend
   */
  private isWeekend(date: Date): boolean {
    const dt = DateTime.fromJSDate(date);
    return dt.weekday === 6 || dt.weekday === 7; // Saturday or Sunday
  }

  /**
   * Format date for job ID
   */
  private formatDate(date: Date): string {
    return DateTime.fromJSDate(date).toFormat('yyyy-MM-dd');
  }

  /**
   * Calculate job priority
   */
  private calculateJobPriority(symbol: string, date: Date): number {
    let priority = 0;

    // Priority symbols get highest priority
    const symbolIndex = this.config.prioritySymbols.indexOf(symbol);
    if (symbolIndex !== -1) {
      priority += 1000 - symbolIndex * 10; // Earlier in list = higher priority
    }

    // Recent dates get higher priority
    const daysAgo = Math.floor(
      (Date.now() - date.getTime()) / (1000 * 60 * 60 * 24)
    );
    priority += Math.max(0, 365 - daysAgo);

    return priority;
  }

  /**
   * Prioritize jobs based on multiple factors
   */
  private prioritizeJobs(jobs: DownloadJob[]): DownloadJob[] {
    return jobs.sort((a, b) => {
      // First sort by priority score
      if (a.priority !== b.priority) {
        return b.priority - a.priority;
      }

      // Then by symbol (alphabetical)
      if (a.symbol !== b.symbol) {
        return a.symbol.localeCompare(b.symbol);
      }

      // Finally by date (recent first)
      return b.date.localeCompare(a.date);
    });
  }

  /**
   * Create optimized batches
   */
  private createBatches(jobs: DownloadJob[]): DownloadJob[][] {
    const batches: DownloadJob[][] = [];
    
    for (let i = 0; i < jobs.length; i += this.config.batchSize) {
      batches.push(jobs.slice(i, i + this.config.batchSize));
    }

    return batches;
  }

  /**
   * Wait until within download time window
   */
  private async waitForDownloadWindow(): Promise<void> {
    const { startHour, endHour } = this.config.downloadTimeWindow;
    
    if (startHour === 0 && endHour === 24) {
      return; // No time restrictions
    }

    while (true) {
      const now = DateTime.now();
      const currentHour = now.hour;
      
      // Handle window that spans midnight
      const inWindow = endHour > startHour
        ? currentHour >= startHour && currentHour < endHour
        : currentHour >= startHour || currentHour < endHour;
      
      if (inWindow) {
        break;
      }

      // Calculate wait time until window opens
      let waitHours: number;
      if (currentHour < startHour) {
        waitHours = startHour - currentHour;
      } else {
        waitHours = 24 - currentHour + startHour;
      }

      const waitMs = waitHours * 60 * 60 * 1000;
      
      this.emit('waitingForWindow', {
        currentHour,
        windowStart: startHour,
        windowEnd: endHour,
        waitTimeHours: waitHours
      });

      // Wait in 5-minute intervals to allow for pause/cancel
      const checkInterval = 5 * 60 * 1000;
      const endTime = Date.now() + waitMs;
      
      while (Date.now() < endTime && !this.isPaused) {
        await this.sleep(Math.min(checkInterval, endTime - Date.now()));
      }
    }
  }

  /**
   * Adjust concurrency based on bandwidth usage
   */
  private async adjustConcurrencyForBandwidth(): Promise<void> {
    if (!this.config.maxBandwidthMbps || this.config.maxBandwidthMbps === Infinity) {
      return;
    }

    const stats = this.downloader.getStatistics();
    const currentBandwidth = stats.averageThroughputMbps;
    
    // Record bandwidth history
    this.bandwidthHistory.push({
      timestamp: Date.now(),
      mbps: currentBandwidth
    });
    
    // Keep only last 5 minutes
    const cutoff = Date.now() - 5 * 60 * 1000;
    this.bandwidthHistory = this.bandwidthHistory.filter(h => h.timestamp > cutoff);
    
    // Calculate average bandwidth
    const avgBandwidth = this.bandwidthHistory.length > 0
      ? this.bandwidthHistory.reduce((sum, h) => sum + h.mbps, 0) / this.bandwidthHistory.length
      : 0;

    // Adjust concurrency if needed
    if (avgBandwidth > this.config.maxBandwidthMbps * 0.9) {
      // Reduce concurrency
      const newConcurrency = Math.max(1, Math.floor(this.config.maxConcurrentDownloads * 0.8));
      this.downloader.setConcurrency(newConcurrency);
      
      this.emit('concurrencyAdjusted', {
        reason: 'bandwidth_limit',
        oldConcurrency: this.config.maxConcurrentDownloads,
        newConcurrency,
        currentBandwidthMbps: avgBandwidth
      });
    } else if (avgBandwidth < this.config.maxBandwidthMbps * 0.5) {
      // Increase concurrency
      const newConcurrency = Math.min(
        this.config.maxConcurrentDownloads,
        Math.ceil(this.downloader.getStatus().statistics.activeJobs * 1.2)
      );
      
      if (newConcurrency > this.downloader.getStatus().statistics.activeJobs) {
        this.downloader.setConcurrency(newConcurrency);
        
        this.emit('concurrencyAdjusted', {
          reason: 'bandwidth_available',
          oldConcurrency: this.downloader.getStatus().statistics.activeJobs,
          newConcurrency,
          currentBandwidthMbps: avgBandwidth
        });
      }
    }
  }

  /**
   * Emit progress update
   */
  private emitProgress(): void {
    const totalJobs = this.completedJobs + this.failedJobs;
    const remainingJobs = (this.totalBatches - this.currentBatch) * this.config.batchSize;
    const avgTimePerJob = this.startTime
      ? (Date.now() - this.startTime.getTime()) / totalJobs / 1000
      : 0;
    
    const progress: SchedulerProgress = {
      totalJobs: totalJobs + remainingJobs,
      completedJobs: this.completedJobs,
      failedJobs: this.failedJobs,
      currentBatch: this.currentBatch,
      totalBatches: this.totalBatches,
      estimatedTimeRemaining: remainingJobs * avgTimePerJob,
      currentThroughputMbps: this.downloader.getStatistics().averageThroughputMbps
    };

    this.emit('progress', progress);
  }

  /**
   * Set up event handlers
   */
  private setupEventHandlers(): void {
    // Forward downloader events
    this.downloader.on('progress', (progress) => {
      this.emit('downloadProgress', progress);
    });

    this.downloader.on('jobComplete', (job, result) => {
      this.emit('jobComplete', job, result);
    });

    this.downloader.on('jobFailed', (job, error) => {
      this.emit('jobFailed', job, error);
    });

    this.downloader.on('jobRetry', (job, error, attempt) => {
      this.emit('jobRetry', job, error, attempt);
    });
  }

  /**
   * Pause downloads
   */
  pause(): void {
    this.isPaused = true;
    this.emit('paused');
  }

  /**
   * Resume downloads
   */
  resume(): void {
    this.isPaused = false;
    this.emit('resumed');
  }

  /**
   * Cancel all downloads
   */
  async cancel(): Promise<void> {
    this.isRunning = false;
    this.isPaused = false;
    await this.downloader.cancelActiveDownloads();
    this.emit('cancelled');
  }

  /**
   * Sleep helper
   */
  private sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  /**
   * Update configuration
   */
  updateConfig(config: Partial<SchedulerConfig>): void {
    Object.assign(this.config, config);
    
    if (config.maxConcurrentDownloads !== undefined) {
      this.downloader.setConcurrency(config.maxConcurrentDownloads);
    }
  }

  /**
   * Get scheduler status
   */
  getStatus(): {
    isRunning: boolean;
    isPaused: boolean;
    currentBatch: number;
    totalBatches: number;
    completedJobs: number;
    failedJobs: number;
    totalRecords: number;
    downloaderStatus: ReturnType<ConcurrentDownloadManager['getStatus']>;
  } {
    return {
      isRunning: this.isRunning,
      isPaused: this.isPaused,
      currentBatch: this.currentBatch,
      totalBatches: this.totalBatches,
      completedJobs: this.completedJobs,
      failedJobs: this.failedJobs,
      totalRecords: this.totalRecords,
      downloaderStatus: this.downloader.getStatus()
    };
  }
}