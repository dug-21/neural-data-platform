# Historical Data Validation System

## Overview

The Historical Data Validation System provides comprehensive validation for market data backfill operations. It ensures data integrity, quality, and completeness throughout the entire backfill process.

## Components

### 1. Pre-Load Validator (`pre_loader.py`)

Validates data **before** database insertion:
- **Format Validation**: Ensures all required fields are present and correctly typed
- **OHLC Consistency**: Verifies High ≥ Low, High ≥ Open/Close, Low ≤ Open/Close
- **Timestamp Validation**: Checks for future dates, duplicates, and proper alignment
- **Price Range Validation**: Detects invalid prices (zero, negative, extreme outliers)
- **Volume Validation**: Ensures non-negative volumes and detects anomalies
- **Continuity Checks**: Identifies gaps in time series data

### 2. Post-Load Validator (`post_loader.py`)

Validates data **after** database insertion:
- **Record Count Verification**: Compares actual vs expected record counts
- **Data Completeness**: Checks for NULL values and missing fields
- **Database Integrity**: Verifies OHLC consistency in stored data
- **Duplicate Detection**: Identifies duplicate timestamps
- **Gap Analysis**: Detects missing time periods
- **Aggregation Consistency**: Validates continuous aggregates match raw data
- **Index Health**: Monitors database index usage and performance

### 3. Gap Detector (`gap_detector.py`)

Specialized gap analysis for time series data:
- **Gap Classification**: Categorizes gaps (weekend, holiday, after-hours, unexpected)
- **Severity Assessment**: Rates gaps from EXPECTED to CRITICAL
- **Market Hours Awareness**: Distinguishes between trading and non-trading gaps
- **Coverage Calculation**: Computes overall data coverage percentage
- **Gap Statistics**: Provides detailed metrics on gap distribution

### 4. Checksum Validator (`checksum_validator.py`)

Ensures data integrity during transfer:
- **MD5 Checksums**: Cryptographic hash for data blocks
- **Statistical Fingerprinting**: Statistical properties as checksums
- **Row Count Verification**: Exact count matching
- **Cross-Provider Validation**: Consistency checks across data sources
- **Anomaly Detection**: Statistical outlier identification

### 5. Data Quality Analyzer (`data_quality.py`)

Comprehensive quality assessment across five dimensions:
- **Completeness** (25% weight): Missing data, NULL values, gaps
- **Accuracy** (25% weight): Price validity, OHLC consistency, outliers
- **Consistency** (20% weight): Cross-provider agreement, OHLC rules
- **Timeliness** (15% weight): Data freshness, insertion delays
- **Validity** (15% weight): Business rules, data patterns

### 6. Validation Report Generator (`validation_report.py`)

Creates comprehensive validation reports:
- **HTML Reports**: Interactive, styled reports with charts
- **JSON Export**: Machine-readable detailed results
- **Summary Reports**: Quick text summaries
- **Dashboard Data**: Aggregated metrics for monitoring

## Usage

### Basic Validation Workflow

```python
import asyncio
from datetime import datetime, timedelta
from data_ingestion.validation import (
    PreLoadValidator,
    PostLoadValidator,
    GapDetector,
    ChecksumValidator,
    DataQualityAnalyzer,
    ValidationReportGenerator,
    ValidationReport
)

async def validate_backfill(symbol: str, start_date: datetime, end_date: datetime):
    """Complete validation workflow for historical data backfill."""
    
    # Initialize validators
    pre_validator = PreLoadValidator()
    post_validator = PostLoadValidator("postgresql://user:pass@localhost/tradingdb")
    gap_detector = GapDetector("postgresql://user:pass@localhost/tradingdb")
    checksum_validator = ChecksumValidator("postgresql://user:pass@localhost/tradingdb")
    quality_analyzer = DataQualityAnalyzer("postgresql://user:pass@localhost/tradingdb")
    report_generator = ValidationReportGenerator()
    
    # Create validation report
    report = ValidationReport(
        report_id=f"{symbol}_{datetime.now().strftime('%Y%m%d_%H%M%S')}",
        symbol=symbol,
        start_date=start_date,
        end_date=end_date,
        validation_timestamp=datetime.now()
    )
    
    # 1. Pre-load validation
    print("Running pre-load validation...")
    pre_result = await pre_validator.validate_batch(
        data_batch,  # Your market data
        granularity=DataGranularity.MINUTE
    )
    report.pre_validation = pre_result
    
    # 2. Load data (your backfill process)
    # ... load data into database ...
    
    # 3. Post-load validation
    print("Running post-load validation...")
    async with post_validator as validator:
        post_result = await validator.validate_load(
            symbol=symbol,
            start_date=start_date,
            end_date=end_date,
            expected_records=expected_count
        )
    report.post_validation = post_result
    
    # 4. Gap analysis
    print("Running gap analysis...")
    async with gap_detector as detector:
        gap_result = await detector.analyze_gaps(
            symbol=symbol,
            start_date=start_date,
            end_date=end_date,
            granularity='1min'
        )
    report.gap_analysis = gap_result
    
    # 5. Checksum validation
    print("Running checksum validation...")
    async with checksum_validator as validator:
        integrity_result = await validator.validate_integrity(
            symbol=symbol,
            start_date=start_date,
            end_date=end_date,
            validate_cross_provider=True
        )
    report.integrity_check = integrity_result
    
    # 6. Quality analysis
    print("Running quality analysis...")
    async with quality_analyzer as analyzer:
        quality_result = await analyzer.analyze_quality(
            symbol=symbol,
            start_date=start_date,
            end_date=end_date,
            granularity='1min'
        )
    report.quality_analysis = quality_result
    
    # 7. Generate report
    print("Generating validation report...")
    html_path = report_generator.generate_report(report, output_format='html')
    json_path = report_generator.generate_report(report, output_format='json')
    summary_path = report_generator.generate_report(report, output_format='summary')
    
    print(f"Validation complete!")
    print(f"Overall Status: {report.overall_status}")
    print(f"Overall Score: {report.overall_score:.1f}%")
    print(f"Reports saved to: {html_path.parent}")
    
    return report

# Run validation
if __name__ == "__main__":
    symbol = "AAPL"
    end_date = datetime.now()
    start_date = end_date - timedelta(days=30)
    
    report = asyncio.run(validate_backfill(symbol, start_date, end_date))
```

### Pre-Load Validation Only

```python
# Validate data before loading
validator = PreLoadValidator()
result = await validator.validate_batch(
    data_batch=market_data_list,
    granularity=DataGranularity.MINUTE,
    check_continuity=True
)

if result.validation_score < 95.0:
    print(f"Data quality too low: {result.validation_score:.1f}%")
    print(f"Errors: {len(result.validation_errors)}")
    for error in result.validation_errors[:10]:
        print(f"  - {error}")
```

### Gap Detection

```python
# Analyze gaps in existing data
async with GapDetector(db_url) as detector:
    result = await detector.analyze_gaps(
        symbol="AAPL",
        start_date=start_date,
        end_date=end_date,
        granularity='1min',
        detailed_analysis=True
    )
    
    # Generate gap report
    report_text = await detector.generate_gap_report(result, output_format='text')
    print(report_text)
    
    # Check specific gap types
    critical_gaps = [
        gap for gap in result.gap_details 
        if gap.severity == GapSeverity.CRITICAL
    ]
    print(f"Found {len(critical_gaps)} critical gaps")
```

### Data Quality Assessment

```python
# Comprehensive quality analysis
async with DataQualityAnalyzer(db_url) as analyzer:
    report = await analyzer.analyze_quality(
        symbol="AAPL",
        start_date=start_date,
        end_date=end_date,
        expected_records=78000,  # 30 days * 390 minutes * ~6.67
        granularity='1min'
    )
    
    print(f"Quality Grade: {report.quality_grade}")
    print(f"Overall Score: {report.overall_quality_score:.1f}%")
    
    # Check individual dimensions
    for name, dimension in report.dimensions.items():
        print(f"{name.capitalize()}: {dimension.score:.1f}% (weight: {dimension.weight})")
        if dimension.issues:
            print(f"  Issues: {', '.join(dimension.issues[:3])}")
```

### Checksum Generation and Validation

```python
# Generate checksums for future validation
async with ChecksumValidator(db_url) as validator:
    # First run - generate checksums
    result = await validator.validate_integrity(
        symbol="AAPL",
        start_date=start_date,
        end_date=end_date
    )
    
    # Store checksums
    checksums = {
        'md5': result.checksum_results['md5'].actual_value,
        'statistical': result.checksum_results['statistical'].actual_value
    }
    
    # Later - validate against stored checksums
    validation_result = await validator.validate_integrity(
        symbol="AAPL",
        start_date=start_date,
        end_date=end_date,
        expected_checksums=checksums
    )
    
    if not validation_result.checksum_results['md5'].matches:
        print("WARNING: Data has been modified!")
```

## Validation Rules

### Critical Validation Rules (Cause Failure)
1. Zero or negative prices
2. High < Low in OHLC data
3. Negative volumes
4. NULL values in required fields
5. Duplicate timestamps
6. Future timestamps

### Warning-Level Rules
1. Large price movements (>20% in single interval)
2. Volume outliers (>5 standard deviations)
3. Weekend data for non-crypto assets
4. Gaps during market hours
5. Provider price discrepancies >1%

### Quality Scoring

**Grade Thresholds:**
- **A**: 90-100% - Production ready
- **B**: 80-89% - Good quality, minor issues
- **C**: 70-79% - Acceptable, needs improvement
- **D**: 60-69% - Poor quality, significant issues
- **F**: <60% - Failing, not suitable for use

## Performance Considerations

### Optimization Tips
1. **Batch Processing**: Validate data in batches of 10,000-50,000 records
2. **Parallel Validation**: Use asyncio for concurrent validation checks
3. **Index Usage**: Ensure proper database indexes for validation queries
4. **Memory Management**: Stream large datasets instead of loading all at once
5. **Checkpointing**: Save validation progress for resumability

### Resource Requirements
- **CPU**: 2-4 cores for parallel validation
- **Memory**: 4-8GB for typical workloads
- **Database**: Connection pool with 10-20 connections
- **Storage**: 100MB-1GB for validation reports

## Integration Examples

### With Backfill Pipeline

```python
class BackfillPipeline:
    def __init__(self):
        self.pre_validator = PreLoadValidator()
        self.post_validator = PostLoadValidator(db_url)
        
    async def process_batch(self, batch: List[MarketData]):
        # Pre-validate
        pre_result = await self.pre_validator.validate_batch(batch)
        if pre_result.validation_score < 95:
            self.logger.warning(f"Low quality batch: {pre_result.validation_score}%")
            # Optionally reject batch
            
        # Process and load
        await self.load_to_database(batch)
        
        # Post-validate
        async with self.post_validator as validator:
            post_result = await validator.validate_load(
                symbol=batch[0].symbol,
                start_date=batch[0].time,
                end_date=batch[-1].time
            )
            
        return pre_result, post_result
```

### With Monitoring System

```python
# Export metrics to Prometheus
from prometheus_client import Counter, Histogram, Gauge

validation_score_gauge = Gauge(
    'data_quality_score',
    'Current data quality score',
    ['symbol', 'dimension']
)

async def monitor_quality(symbol: str):
    async with DataQualityAnalyzer(db_url) as analyzer:
        report = await analyzer.analyze_quality(symbol, start_date, end_date)
        
        # Update Prometheus metrics
        validation_score_gauge.labels(
            symbol=symbol,
            dimension='overall'
        ).set(report.overall_quality_score)
        
        for name, dimension in report.dimensions.items():
            validation_score_gauge.labels(
                symbol=symbol,
                dimension=name
            ).set(dimension.score)
```

## Troubleshooting

### Common Issues

1. **"No data found for analysis"**
   - Verify database connection
   - Check date range and symbol
   - Ensure data has been loaded

2. **"Validation taking too long"**
   - Reduce batch size
   - Add database indexes
   - Increase connection pool size

3. **"Memory error during validation"**
   - Use streaming for large datasets
   - Process in smaller batches
   - Increase available memory

4. **"Checksum mismatch"**
   - Data may have been modified
   - Check for timezone issues
   - Verify data source consistency

## Best Practices

1. **Always validate before and after loading**
2. **Set appropriate quality thresholds for your use case**
3. **Monitor validation metrics over time**
4. **Archive validation reports for audit trail**
5. **Automate validation in your data pipeline**
6. **Review and act on validation recommendations**
7. **Regularly update validation rules based on findings**

## Future Enhancements

1. **Machine Learning Anomaly Detection**
2. **Real-time Validation Streaming**
3. **Custom Validation Rule Engine**
4. **Integration with Data Lineage Tracking**
5. **Automated Issue Resolution**
6. **Validation Performance Optimization**