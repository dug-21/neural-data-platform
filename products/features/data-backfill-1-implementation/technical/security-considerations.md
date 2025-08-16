# Security Considerations

## Overview

This document outlines security best practices and considerations for the data backfill implementation, covering authentication, data protection, access control, and compliance requirements.

## Authentication & Authorization

### AWS Credentials Management

#### Best Practices
1. **Never hardcode credentials**
   ```python
   # ❌ WRONG - Never do this
   s3_client = boto3.client(
       's3',
       aws_access_key_id='AKIAIOSFODNN7EXAMPLE',
       aws_secret_access_key='wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY'
   )
   
   # ✅ CORRECT - Use profiles or IAM roles
   session = boto3.Session(profile_name='polygon-s3')
   s3_client = session.client('s3')
   ```

2. **Use AWS Profiles**
   ```bash
   # Configure profile with limited permissions
   aws configure --profile polygon-s3
   ```

3. **Implement Credential Rotation**
   ```python
   # Automatic credential refresh
   from boto3.session import Session
   from botocore.credentials import RefreshableCredentials
   
   def get_refreshable_credentials():
       """Get credentials that auto-refresh."""
       session = Session()
       return session.get_credentials()
   ```

### API Key Protection

1. **Environment Variables**
   ```python
   # Load from environment
   import os
   API_KEY = os.environ.get('POLYGON_API_KEY')
   
   if not API_KEY:
       raise ValueError("POLYGON_API_KEY environment variable not set")
   ```

2. **Secure Storage**
   ```python
   # Use keyring for secure storage
   import keyring
   
   # Store securely
   keyring.set_password('neural-trader', 'polygon-api', api_key)
   
   # Retrieve securely
   api_key = keyring.get_password('neural-trader', 'polygon-api')
   ```

## Data Protection

### Encryption

#### In Transit
```python
# Enforce HTTPS/TLS
ssl_context = ssl.create_default_context()
ssl_context.check_hostname = True
ssl_context.verify_mode = ssl.CERT_REQUIRED

# For S3
s3_client = boto3.client(
    's3',
    config=Config(
        signature_version='s3v4',
        s3={'use_ssl': True}
    )
)
```

#### At Rest
```python
# Enable server-side encryption for S3 downloads
response = s3_client.get_object(
    Bucket='flatfiles',
    Key=object_key,
    SSECustomerAlgorithm='AES256'
)

# Local file encryption
from cryptography.fernet import Fernet

def encrypt_file(file_path, key):
    """Encrypt sensitive data files."""
    fernet = Fernet(key)
    with open(file_path, 'rb') as f:
        encrypted = fernet.encrypt(f.read())
    with open(file_path + '.enc', 'wb') as f:
        f.write(encrypted)
```

### Data Validation

#### Input Validation
```python
def validate_symbol(symbol: str) -> bool:
    """Validate stock symbol format."""
    if not symbol:
        return False
    # Allow only alphanumeric and specific chars
    if not re.match(r'^[A-Z0-9\-\.]{1,10}$', symbol):
        return False
    return True

def validate_date_range(start: datetime, end: datetime) -> bool:
    """Validate date range is reasonable."""
    # Prevent excessive data requests
    max_days = 365 * 5  # 5 years max
    if (end - start).days > max_days:
        raise ValueError(f"Date range exceeds maximum of {max_days} days")
    return True
```

#### Data Integrity
```python
def verify_file_checksum(file_path: str, expected_checksum: str) -> bool:
    """Verify file integrity using SHA-256."""
    sha256_hash = hashlib.sha256()
    with open(file_path, 'rb') as f:
        for byte_block in iter(lambda: f.read(4096), b""):
            sha256_hash.update(byte_block)
    
    calculated = sha256_hash.hexdigest()
    return calculated == expected_checksum
```

## Access Control

### Principle of Least Privilege

1. **S3 Bucket Policy**
   ```json
   {
     "Version": "2012-10-17",
     "Statement": [
       {
         "Effect": "Allow",
         "Principal": {
           "AWS": "arn:aws:iam::123456789012:user/backfill-user"
         },
         "Action": [
           "s3:GetObject",
           "s3:ListBucket"
         ],
         "Resource": [
           "arn:aws:s3:::flatfiles/us_stocks_sip/*"
         ],
         "Condition": {
           "IpAddress": {
             "aws:SourceIp": ["10.0.0.0/8"]
           }
         }
       }
     ]
   }
   ```

2. **Database Access**
   ```python
   # Create read-only user for backfill
   CREATE USER backfill_user WITH PASSWORD 'strong_password';
   GRANT CONNECT ON DATABASE trading TO backfill_user;
   GRANT USAGE ON SCHEMA public TO backfill_user;
   GRANT INSERT ON TABLE market_data TO backfill_user;
   -- No UPDATE or DELETE permissions
   ```

### Rate Limiting

```python
from utils.rate_limiter import RateLimiter

# Implement rate limiting to prevent abuse
rate_limiter = RateLimiter(
    max_requests=100,
    time_window=60  # 100 requests per minute
)

@rate_limiter.limit
async def download_file(s3_key):
    """Rate-limited download function."""
    return await s3_client.download_file(...)
```

## Audit & Compliance

### Logging

#### Security Events
```python
import logging
from datetime import datetime

security_logger = logging.getLogger('security')

def log_security_event(event_type: str, details: dict):
    """Log security-relevant events."""
    security_logger.info(json.dumps({
        'timestamp': datetime.utcnow().isoformat(),
        'event_type': event_type,
        'user': os.environ.get('USER'),
        'details': details
    }))

# Log authentication attempts
log_security_event('auth_attempt', {
    'service': 's3',
    'profile': 'polygon-s3',
    'success': True
})

# Log data access
log_security_event('data_access', {
    'resource': s3_key,
    'action': 'download',
    'size_bytes': file_size
})
```

#### Audit Trail
```python
class AuditLogger:
    """Maintain audit trail of all operations."""
    
    def __init__(self, log_file: str):
        self.logger = self._setup_logger(log_file)
    
    def log_operation(self, operation: str, **kwargs):
        self.logger.info(json.dumps({
            'timestamp': datetime.utcnow().isoformat(),
            'operation': operation,
            'user': os.environ.get('USER'),
            'host': socket.gethostname(),
            **kwargs
        }))
```

### Compliance

#### Data Retention
```python
# Implement data retention policies
class DataRetentionManager:
    def __init__(self, retention_days: int = 2555):  # 7 years default
        self.retention_days = retention_days
    
    def should_delete(self, file_date: datetime) -> bool:
        """Check if file should be deleted per retention policy."""
        age_days = (datetime.utcnow() - file_date).days
        return age_days > self.retention_days
```

#### GDPR Compliance
```python
# Anonymize any PII in logs
def anonymize_ip(ip_address: str) -> str:
    """Anonymize IP address for GDPR compliance."""
    parts = ip_address.split('.')
    if len(parts) == 4:
        parts[-1] = '0'  # Zero out last octet
    return '.'.join(parts)
```

## Error Handling

### Secure Error Messages
```python
class SecureError(Exception):
    """Base class for secure error handling."""
    
    def __init__(self, message: str, details: dict = None):
        # Log detailed error internally
        logger.error(f"{message}: {details}")
        
        # Return generic message to user
        super().__init__(message)

# Usage
try:
    s3_client.download_file(...)
except ClientError as e:
    # Don't expose S3 paths or internal details
    raise SecureError(
        "Failed to download market data",
        details={'error': str(e), 's3_key': s3_key}
    )
```

## Network Security

### TLS Configuration
```python
# Enforce minimum TLS version
ssl_context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
ssl_context.minimum_version = ssl.TLSVersion.TLSv1_2
ssl_context.maximum_version = ssl.TLSVersion.TLSv1_3
```

### Proxy Support
```python
# Support corporate proxies with authentication
proxies = {
    'http': 'http://user:pass@proxy.corp.com:8080',
    'https': 'http://user:pass@proxy.corp.com:8080'
}

# Configure boto3 with proxy
os.environ['HTTP_PROXY'] = proxies['http']
os.environ['HTTPS_PROXY'] = proxies['https']
```

## Container Security

### Docker Best Practices
```dockerfile
# Run as non-root user
FROM python:3.9-slim

# Create non-root user
RUN groupadd -r backfill && useradd -r -g backfill backfill

# Set secure permissions
RUN mkdir /app && chown backfill:backfill /app
WORKDIR /app

# Copy only necessary files
COPY --chown=backfill:backfill requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Switch to non-root user
USER backfill

# Don't expose unnecessary ports
# EXPOSE is intentionally omitted
```

### Secrets Management
```yaml
# docker-compose.yml
version: '3.8'

services:
  backfill:
    image: neural-trader/backfill
    environment:
      - AWS_PROFILE=polygon-s3
    secrets:
      - polygon_api_key
      - db_password
    
secrets:
  polygon_api_key:
    external: true
  db_password:
    external: true
```

## Security Checklist

### Pre-Deployment
- [ ] All credentials in environment variables or secure storage
- [ ] No hardcoded secrets in code
- [ ] TLS/HTTPS enforced for all connections
- [ ] Input validation implemented
- [ ] Rate limiting configured
- [ ] Audit logging enabled
- [ ] Error messages sanitized

### Runtime
- [ ] Monitor for unauthorized access attempts
- [ ] Regular credential rotation
- [ ] Security patches applied
- [ ] Audit logs reviewed
- [ ] Access patterns analyzed
- [ ] Anomaly detection active

### Post-Deployment
- [ ] Security scan results reviewed
- [ ] Penetration testing completed
- [ ] Compliance requirements verified
- [ ] Incident response plan tested
- [ ] Data retention policies enforced

## Incident Response

### Security Incident Procedure
1. **Detect** - Monitor logs for anomalies
2. **Contain** - Isolate affected systems
3. **Investigate** - Analyze root cause
4. **Remediate** - Fix vulnerabilities
5. **Document** - Record lessons learned

### Emergency Contacts
- Security Team: security@company.com
- On-Call Engineer: +1-XXX-XXX-XXXX
- AWS Support: https://console.aws.amazon.com/support