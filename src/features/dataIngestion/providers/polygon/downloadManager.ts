import { EventEmitter } from 'events';
import { PolygonS3Client, S3FileInfo } from './s3Client';
import { GzippedCSVParser } from './csvParser';
import { PolygonDataTransformer, TimeSeriesData } from './dataTransformer';
import pLimit from 'p-limit';
import { backOff } from 'exponential-backoff';

export interface DownloadJob {
  id: string;
  symbol: string;
  date: string;
  s3Key: string;
  fileSize?: number;
  priority: number;
  retries: number;
  status: 'pending' | 'downloading' | 'processing' | 'completed' | 'failed';
  error?: Error;
  startTime?: Date;
  endTime?: Date;
  recordsProcessed?: number;
  bytesDownloaded?: number;
}

export interface DownloadProgress {
  jobId: string;
  status: string;
  bytesDownloaded: number;
  totalBytes: number;
  recordsProcessed: number;
  percentComplete: number;
  throughputMbps?: number;
}

export interface DownloadResult {
  jobId: string;
  success: boolean;
  recordsProcessed: number;
  dataPoints: TimeSeriesData[];
  duration: number;
  error?: Error;
}

export interface DownloadManagerConfig {
  maxConcurrency?: number;
  maxRetries?: number;
  batchSize?: number;
  retryDelayMs?: number;
  timeoutMs?: number;
  validateData?: boolean;
}

export interface DownloadStatistics {
  totalJobs: number;
  completedJobs: number;
  failedJobs: number;
  activeJobs: number;
  totalBytesDownloaded: number;
  totalRecordsProcessed: number;
  averageThroughputMbps: number;
  totalDurationSeconds: number;
}

/**
 * Manages concurrent downloads from S3 with retry logic and progress tracking
 */
export class ConcurrentDownloadManager extends EventEmitter {
  private s3Client: PolygonS3Client;
  private parser: GzippedCSVParser;
  private config: Required<DownloadManagerConfig>;
  private concurrencyLimit: pLimit.Limit;
  
  // Job tracking
  private jobs: Map<string, DownloadJob> = new Map();
  private activeJobs: Set<string> = new Set();
  private completedJobs: Set<string> = new Set();
  private failedJobs: Map<string, Error> = new Map();
  
  // Statistics
  private totalBytesDownloaded: number = 0;
  private totalRecordsProcessed: number = 0;
  private startTime: Date | null = null;
  private throughputMeasurements: { timestamp: number; bytes: number }[] = [];

  constructor(
    s3Client: PolygonS3Client,
    config: DownloadManagerConfig = {}
  ) {
    super();
    this.s3Client = s3Client;
    
    // Set default configuration
    this.config = {
      maxConcurrency: config.maxConcurrency || 10,
      maxRetries: config.maxRetries || 3,
      batchSize: config.batchSize || 5000,
      retryDelayMs: config.retryDelayMs || 1000,
      timeoutMs: config.timeoutMs || 300000, // 5 minutes
      validateData: config.validateData !== false
    };
    
    this.parser = new GzippedCSVParser({ 
      batchSize: this.config.batchSize,
      skipInvalid: true
    });
    
    this.concurrencyLimit = pLimit(this.config.maxConcurrency);
  }

  /**
   * Download multiple files concurrently
   */
  async downloadBatch(jobs: DownloadJob[]): Promise<Map<string, DownloadResult>> {
    if (!this.startTime) {
      this.startTime = new Date();
    }

    // Add jobs to tracking
    for (const job of jobs) {
      this.jobs.set(job.id, job);
    }

    // Sort jobs by priority (higher priority first)
    const sortedJobs = [...jobs].sort((a, b) => b.priority - a.priority);
    
    // Create download promises with concurrency limit
    const downloadPromises = sortedJobs.map(job => 
      this.concurrencyLimit(() => this.downloadWithRetry(job))
    );

    // Wait for all downloads to complete
    const results = await Promise.allSettled(downloadPromises);
    
    // Process results
    const resultMap = new Map<string, DownloadResult>();
    
    for (let i = 0; i < results.length; i++) {
      const result = results[i];
      const job = sortedJobs[i];
      
      if (result.status === 'fulfilled') {
        resultMap.set(job.id, result.value);
      } else {
        resultMap.set(job.id, {
          jobId: job.id,
          success: false,
          recordsProcessed: 0,
          dataPoints: [],
          duration: 0,
          error: result.reason
        });
      }
    }

    // Emit batch completion event
    this.emit('batchComplete', {
      totalJobs: jobs.length,
      successful: Array.from(resultMap.values()).filter(r => r.success).length,
      failed: Array.from(resultMap.values()).filter(r => !r.success).length,
      statistics: this.getStatistics()
    });

    return resultMap;
  }

  /**
   * Download a single job with retry logic
   */
  private async downloadWithRetry(job: DownloadJob): Promise<DownloadResult> {
    // Update job status
    job.status = 'downloading';
    job.startTime = new Date();
    this.activeJobs.add(job.id);
    this.emit('jobStart', job);

    try {
      // Use exponential backoff for retries
      const result = await backOff(
        () => this.downloadSingleFile(job),
        {
          numOfAttempts: this.config.maxRetries,
          startingDelay: this.config.retryDelayMs,
          timeMultiple: 2,
          maxDelay: 30000,
          jitter: 'full',
          retry: (error: any, attemptNumber: number) => {
            job.retries = attemptNumber;
            this.emit('jobRetry', job, error, attemptNumber);
            
            // Don't retry on certain errors
            if (error.code === 'NoSuchKey' || error.code === 'AccessDenied') {
              return false;
            }
            
            return attemptNumber < this.config.maxRetries;
          }
        }
      );

      // Update job status
      job.status = 'completed';
      job.endTime = new Date();
      job.recordsProcessed = result.recordsProcessed;
      
      this.completedJobs.add(job.id);
      this.activeJobs.delete(job.id);
      
      this.emit('jobComplete', job, result);
      
      return result;
    } catch (error) {
      // Handle failure
      job.status = 'failed';
      job.endTime = new Date();
      job.error = error as Error;
      
      this.failedJobs.set(job.id, error as Error);
      this.activeJobs.delete(job.id);
      
      this.emit('jobFailed', job, error);
      
      throw error;
    }
  }

  /**
   * Download and process a single file
   */
  private async downloadSingleFile(job: DownloadJob): Promise<DownloadResult> {
    const startTime = Date.now();
    let bytesDownloaded = 0;
    let recordsProcessed = 0;
    const allData: TimeSeriesData[] = [];

    try {
      // Get file metadata if not provided
      if (!job.fileSize) {
        const metadata = await this.s3Client.getFileMetadata(job.s3Key);
        job.fileSize = metadata.size;
      }

      // Download stream
      const stream = await this.s3Client.downloadStream(job.s3Key);
      
      // Set up data transformer
      const transformer = new PolygonDataTransformer({
        symbol: job.symbol,
        validateData: this.config.validateData
      });

      // Track download progress
      let lastProgressUpdate = Date.now();
      
      stream.on('data', (chunk: Buffer) => {
        bytesDownloaded += chunk.length;
        this.totalBytesDownloaded += chunk.length;
        
        // Update throughput measurements
        this.throughputMeasurements.push({
          timestamp: Date.now(),
          bytes: chunk.length
        });
        
        // Emit progress updates every 100ms
        if (Date.now() - lastProgressUpdate > 100) {
          const progress: DownloadProgress = {
            jobId: job.id,
            status: 'downloading',
            bytesDownloaded,
            totalBytes: job.fileSize || 0,
            recordsProcessed,
            percentComplete: job.fileSize ? (bytesDownloaded / job.fileSize) * 100 : 0,
            throughputMbps: this.calculateThroughput()
          };
          
          job.bytesDownloaded = bytesDownloaded;
          this.emit('progress', progress);
          lastProgressUpdate = Date.now();
        }
      });

      // Parse CSV data in batches
      job.status = 'processing';
      
      for await (const batch of this.parser.parseAggregatesStream(stream)) {
        const transformedData = transformer.transformMarketDataBatch(batch);
        allData.push(...transformedData);
        recordsProcessed += batch.length;
        this.totalRecordsProcessed += batch.length;
        
        // Emit processing progress
        if (recordsProcessed % 10000 === 0) {
          const progress: DownloadProgress = {
            jobId: job.id,
            status: 'processing',
            bytesDownloaded,
            totalBytes: job.fileSize || 0,
            recordsProcessed,
            percentComplete: 100,
            throughputMbps: this.calculateThroughput()
          };
          
          job.recordsProcessed = recordsProcessed;
          this.emit('progress', progress);
        }
      }

      // Get validation summary if available
      const validationSummary = transformer.getValidationSummary();
      if (validationSummary.size > 0) {
        console.warn(`Validation errors for ${job.id}:`, 
          Array.from(validationSummary.entries())
        );
      }

      const duration = (Date.now() - startTime) / 1000;
      
      return {
        jobId: job.id,
        success: true,
        recordsProcessed,
        dataPoints: allData,
        duration
      };
    } catch (error) {
      console.error(`Error downloading ${job.id}:`, error);
      throw error;
    }
  }

  /**
   * Calculate current download throughput
   */
  private calculateThroughput(): number {
    // Clean up old measurements (keep last 5 seconds)
    const cutoff = Date.now() - 5000;
    this.throughputMeasurements = this.throughputMeasurements.filter(
      m => m.timestamp > cutoff
    );

    if (this.throughputMeasurements.length < 2) {
      return 0;
    }

    // Calculate throughput from recent measurements
    const totalBytes = this.throughputMeasurements.reduce(
      (sum, m) => sum + m.bytes, 
      0
    );
    
    const timeSpan = 
      this.throughputMeasurements[this.throughputMeasurements.length - 1].timestamp -
      this.throughputMeasurements[0].timestamp;
    
    if (timeSpan === 0) return 0;
    
    // Convert to Mbps
    return (totalBytes * 8) / (timeSpan * 1000);
  }

  /**
   * Get current download manager status
   */
  getStatus(): {
    jobs: Map<string, DownloadJob>;
    activeJobs: string[];
    completedJobs: string[];
    failedJobs: Map<string, Error>;
    statistics: DownloadStatistics;
  } {
    return {
      jobs: new Map(this.jobs),
      activeJobs: Array.from(this.activeJobs),
      completedJobs: Array.from(this.completedJobs),
      failedJobs: new Map(this.failedJobs),
      statistics: this.getStatistics()
    };
  }

  /**
   * Get download statistics
   */
  getStatistics(): DownloadStatistics {
    const totalDuration = this.startTime ? 
      (Date.now() - this.startTime.getTime()) / 1000 : 0;
    
    return {
      totalJobs: this.jobs.size,
      completedJobs: this.completedJobs.size,
      failedJobs: this.failedJobs.size,
      activeJobs: this.activeJobs.size,
      totalBytesDownloaded: this.totalBytesDownloaded,
      totalRecordsProcessed: this.totalRecordsProcessed,
      averageThroughputMbps: totalDuration > 0 ? 
        (this.totalBytesDownloaded * 8) / (totalDuration * 1000000) : 0,
      totalDurationSeconds: totalDuration
    };
  }

  /**
   * Cancel active downloads
   */
  async cancelActiveDownloads(): Promise<void> {
    // Note: Actual cancellation would require AbortController support
    // For now, we just mark jobs as failed
    for (const jobId of this.activeJobs) {
      const job = this.jobs.get(jobId);
      if (job) {
        job.status = 'failed';
        job.error = new Error('Download cancelled');
        this.failedJobs.set(jobId, job.error);
      }
    }
    
    this.activeJobs.clear();
    this.emit('cancelled', this.activeJobs.size);
  }

  /**
   * Reset download manager state
   */
  reset(): void {
    this.jobs.clear();
    this.activeJobs.clear();
    this.completedJobs.clear();
    this.failedJobs.clear();
    this.totalBytesDownloaded = 0;
    this.totalRecordsProcessed = 0;
    this.startTime = null;
    this.throughputMeasurements = [];
    this.parser.reset();
  }

  /**
   * Update concurrency limit dynamically
   */
  setConcurrency(maxConcurrency: number): void {
    this.config.maxConcurrency = maxConcurrency;
    this.concurrencyLimit = pLimit(maxConcurrency);
  }
}