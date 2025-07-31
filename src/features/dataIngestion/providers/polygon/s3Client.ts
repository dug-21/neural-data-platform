import { S3Client, GetObjectCommand, ListObjectsV2Command, HeadObjectCommand } from '@aws-sdk/client-s3';
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

export interface S3FileInfo {
  key: string;
  size: number;
  lastModified: Date;
  etag?: string;
}

export interface S3DownloadProgress {
  bytesDownloaded: number;
  totalBytes: number;
  percentComplete: number;
}

/**
 * Wrapper for AWS S3 client specifically configured for Polygon flat files
 */
export class PolygonS3Client {
  private client: S3Client;
  private config: S3Config;

  constructor(config: Partial<S3Config> = {}) {
    this.config = {
      bucketName: 'polygon-flat-files',
      region: 'us-east-1',
      maxRetries: 3,
      requestTimeout: 30000,
      ...config
    };

    // Initialize S3 client with credentials if provided
    const clientConfig: any = {
      region: this.config.region,
      maxAttempts: this.config.maxRetries,
      requestHandler: {
        requestTimeout: this.config.requestTimeout
      }
    };

    // Add credentials if provided
    if (this.config.accessKeyId && this.config.secretAccessKey) {
      clientConfig.credentials = {
        accessKeyId: this.config.accessKeyId,
        secretAccessKey: this.config.secretAccessKey
      };
    }

    // Add custom endpoint if provided (for S3-compatible services)
    if (this.config.endpoint) {
      clientConfig.endpoint = this.config.endpoint;
      clientConfig.forcePathStyle = true;
    }

    this.client = new S3Client(clientConfig);
  }

  /**
   * List files in S3 bucket with pagination support
   */
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

      try {
        const response = await this.client.send(command);
        
        if (response.Contents) {
          files.push(...response.Contents.map(obj => ({
            key: obj.Key!,
            size: obj.Size || 0,
            lastModified: obj.LastModified || new Date(),
            etag: obj.ETag
          })));
        }

        continuationToken = response.NextContinuationToken;
      } catch (error) {
        console.error(`Error listing files with prefix ${prefix}:`, error);
        throw new Error(`Failed to list S3 files: ${error.message}`);
      }
    } while (continuationToken);

    return files;
  }

  /**
   * Get file metadata without downloading
   */
  async getFileMetadata(key: string): Promise<S3FileInfo> {
    const command = new HeadObjectCommand({
      Bucket: this.config.bucketName,
      Key: key
    });

    try {
      const response = await this.client.send(command);
      
      return {
        key,
        size: response.ContentLength || 0,
        lastModified: response.LastModified || new Date(),
        etag: response.ETag
      };
    } catch (error) {
      throw new Error(`Failed to get metadata for ${key}: ${error.message}`);
    }
  }

  /**
   * Download file as a stream
   */
  async downloadStream(key: string): Promise<Readable> {
    const command = new GetObjectCommand({
      Bucket: this.config.bucketName,
      Key: key
    });

    try {
      const response = await this.client.send(command);
      
      if (!response.Body) {
        throw new Error('No body in S3 response');
      }

      // Convert Body to Node.js Readable stream
      return response.Body as Readable;
    } catch (error) {
      throw new Error(`Failed to download ${key}: ${error.message}`);
    }
  }

  /**
   * Generate presigned URL for direct download
   */
  async generatePresignedUrl(key: string, expiresIn: number = 3600): Promise<string> {
    const command = new GetObjectCommand({
      Bucket: this.config.bucketName,
      Key: key
    });

    try {
      return await getSignedUrl(this.client, command, { expiresIn });
    } catch (error) {
      throw new Error(`Failed to generate presigned URL for ${key}: ${error.message}`);
    }
  }

  /**
   * Build S3 key for Polygon file structure
   */
  static buildS3Key(params: {
    assetClass: 'stocks' | 'options' | 'crypto' | 'forex';
    dataType: 'trades' | 'quotes' | 'aggregates';
    timeframe?: 'minute' | 'hour' | 'day';
    symbol: string;
    date: Date;
  }): string {
    const year = params.date.getFullYear();
    const month = String(params.date.getMonth() + 1).padStart(2, '0');
    const day = String(params.date.getDate()).padStart(2, '0');
    const dateStr = `${year}-${month}-${day}`;

    // Build path based on Polygon's structure
    let path = `${params.assetClass}/${year}/${month}/${day}/`;

    if (params.dataType === 'aggregates' && params.timeframe) {
      path += `aggregates/${params.timeframe}/`;
    }

    return `${path}${params.symbol}_${dateStr}.csv.gz`;
  }

  /**
   * Parse S3 key to extract metadata
   */
  static parseS3Key(key: string): {
    assetClass: string;
    symbol: string;
    date: string;
    dataType: string;
    timeframe?: string;
  } | null {
    // Example: stocks/2023/12/25/aggregates/minute/AAPL_2023-12-25.csv.gz
    const match = key.match(
      /^(\w+)\/(\d{4})\/(\d{2})\/(\d{2})(?:\/aggregates\/(\w+))?\/([A-Z]+)_(\d{4}-\d{2}-\d{2})\.csv\.gz$/
    );

    if (!match) {
      return null;
    }

    const [, assetClass, year, month, day, timeframe, symbol, date] = match;

    return {
      assetClass,
      symbol,
      date,
      dataType: timeframe ? 'aggregates' : 'trades',
      timeframe
    };
  }

  /**
   * Check if bucket is accessible
   */
  async testConnection(): Promise<boolean> {
    try {
      const command = new ListObjectsV2Command({
        Bucket: this.config.bucketName,
        MaxKeys: 1
      });

      await this.client.send(command);
      return true;
    } catch (error) {
      console.error('S3 connection test failed:', error);
      return false;
    }
  }

  /**
   * Cleanup and close connections
   */
  async close(): Promise<void> {
    // S3Client doesn't require explicit cleanup
    // This method is here for interface consistency
  }
}