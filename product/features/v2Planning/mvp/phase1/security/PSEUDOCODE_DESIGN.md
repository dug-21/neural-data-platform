# Security Remediation - Pseudocode Design

## 1. Secret Blocking System

```pseudocode
MODULE SecurityBlocklist:
    CONST BLOCKED_PATTERNS = [
        "password", "passwd", "pwd",
        "secret", "api_key", "apikey", 
        "token", "auth", "credential",
        "private_key", "privatekey",
        "client_secret", "access_token"
    ]
    
    CONST VALUE_PATTERNS = [
        regex("^sk_[a-zA-Z]+_"),     // Stripe keys
        regex("^pk_[a-zA-Z]+_"),     // Public keys
        regex("^[A-Za-z0-9+/]{40,}=*$"), // Base64 secrets
        regex("^ghp_[a-zA-Z0-9]{36}$"),  // GitHub tokens
    ]
    
    FUNCTION is_blocked_key(key: string) -> bool:
        key_lower = key.to_lowercase()
        FOR pattern IN BLOCKED_PATTERNS:
            IF pattern IN key_lower:
                RETURN true
        RETURN false
    
    FUNCTION is_blocked_value(value: string) -> bool:
        FOR pattern IN VALUE_PATTERNS:
            IF pattern.matches(value):
                RETURN true
        RETURN false
    
    FUNCTION check_secret(key: string, value: ConfigValue) -> Result:
        IF is_blocked_key(key):
            RETURN Error("Secrets/passwords cannot be stored in config-store")
        
        IF value IS String:
            IF is_blocked_value(value.content):
                RETURN Error("Value appears to be a secret/credential")
        
        IF value IS Object:
            FOR each (k, v) IN value:
                result = check_secret(k, v)
                IF result IS Error:
                    RETURN result
        
        RETURN Ok()
```

## 2. Safe JSON Deserialization

```pseudocode
MODULE SafeJsonParser:
    CONST MAX_SIZE = 10_485_760  // 10MB
    CONST MAX_DEPTH = 128
    CONST MAX_KEYS = 10000
    
    FUNCTION parse_safe(json_str: string) -> Result<ConfigValue>:
        // Size check
        IF json_str.length > MAX_SIZE:
            RETURN Error("JSON exceeds maximum size of 10MB")
        
        // Pre-parse validation
        depth = calculate_depth(json_str)
        IF depth > MAX_DEPTH:
            RETURN Error("JSON nesting exceeds maximum depth of 128")
        
        // Parse with limits
        TRY:
            value = parse_with_limits(json_str, MAX_KEYS)
            RETURN Ok(value)
        CATCH ParseError as e:
            RETURN Error("Invalid JSON: " + sanitize_error(e))
    
    FUNCTION calculate_depth(json: string) -> int:
        max_depth = 0
        current_depth = 0
        
        FOR char IN json:
            IF char == '{' OR char == '[':
                current_depth += 1
                max_depth = MAX(max_depth, current_depth)
            ELIF char == '}' OR char == ']':
                current_depth -= 1
        
        RETURN max_depth
    
    FUNCTION parse_with_limits(json: string, max_keys: int) -> ConfigValue:
        parser = JsonParser::new()
        parser.set_max_keys(max_keys)
        parser.set_max_string_length(1_000_000)
        RETURN parser.parse(json)
```

## 3. Path Traversal Protection

```pseudocode
MODULE SecureFileLoader:
    allowed_dirs: List<Path>
    
    FUNCTION load_file(file_path: string) -> Result<string>:
        // Convert to Path object
        path = Path::new(file_path)
        
        // Canonicalize (resolves .., symlinks, etc)
        TRY:
            canonical = path.canonicalize()
        CATCH:
            RETURN Error("Invalid path")
        
        // Check if within allowed directories
        is_allowed = false
        FOR allowed_dir IN allowed_dirs:
            IF canonical.starts_with(allowed_dir):
                is_allowed = true
                BREAK
        
        IF NOT is_allowed:
            RETURN Error("Access denied: path outside allowed directories")
        
        // Additional safety checks
        IF canonical.contains(".."):
            RETURN Error("Path traversal detected")
        
        // Read file with size limit
        RETURN read_file_with_limit(canonical, MAX_FILE_SIZE)
```

## 4. Error Sanitization

```pseudocode
MODULE ErrorSanitizer:
    is_production: bool
    
    FUNCTION sanitize(error: Error) -> Error:
        IF is_production:
            RETURN sanitize_for_production(error)
        ELSE:
            RETURN error  // Full details in development
    
    FUNCTION sanitize_for_production(error: Error) -> Error:
        MATCH error.type:
            CASE FileNotFound:
                RETURN Error("Configuration not found")
            CASE PathTraversal:
                RETURN Error("Invalid path")
            CASE ParseError:
                RETURN Error("Invalid configuration format")
            CASE ValidationError:
                RETURN Error("Invalid configuration value")
            CASE RateLimitExceeded:
                RETURN Error("Too many requests")
            DEFAULT:
                RETURN Error("Configuration error")
        
        // Log full error internally
        log_internal(error)
```

## 5. Thread-Safe Async Operations

```pseudocode
MODULE ThreadSafeStore:
    data: Arc<RwLock<HashMap>>
    write_lock: Arc<Mutex<()>>
    
    ASYNC FUNCTION get(key: string) -> Result<ConfigValue>:
        // Read lock for concurrent reads
        read_guard = data.read().await
        value = read_guard.get(key)
        RETURN value.cloned()
    
    ASYNC FUNCTION set(key: string, value: ConfigValue) -> Result:
        // Check for secrets first
        SecurityBlocklist::check_secret(key, value)?
        
        // Validate input
        Validator::validate_key(key)?
        Validator::validate_value(value)?
        
        // Write lock for exclusive write
        write_guard = write_lock.lock().await
        
        // Get write access to data
        mut data_guard = data.write().await
        data_guard.insert(key, value)
        
        RETURN Ok()
    
    ASYNC FUNCTION update_atomic(key: string, updater: Function) -> Result:
        // Exclusive lock for atomic update
        write_guard = write_lock.lock().await
        mut data_guard = data.write().await
        
        current = data_guard.get(key)
        new_value = updater(current)?
        
        // Validate new value
        SecurityBlocklist::check_secret(key, new_value)?
        Validator::validate_value(new_value)?
        
        data_guard.insert(key, new_value)
        RETURN Ok()
```

## 6. Rate Limiting

```pseudocode
MODULE RateLimiter:
    buckets: HashMap<ClientId, TokenBucket>
    max_tokens: int
    refill_rate: Duration
    
    FUNCTION check_limit(client_id: string) -> Result:
        bucket = buckets.get_or_create(client_id, max_tokens)
        
        // Refill tokens based on time elapsed
        elapsed = now() - bucket.last_refill
        tokens_to_add = (elapsed / refill_rate) * max_tokens
        bucket.tokens = MIN(max_tokens, bucket.tokens + tokens_to_add)
        bucket.last_refill = now()
        
        // Check if request allowed
        IF bucket.tokens >= 1:
            bucket.tokens -= 1
            RETURN Ok()
        ELSE:
            RETURN Error("Rate limit exceeded")
    
    FUNCTION reset(client_id: string):
        buckets.remove(client_id)
```

## 7. Input Validation

```pseudocode
MODULE EnhancedValidator:
    CONST KEY_PATTERN = regex("^[a-zA-Z0-9_.-]+(/[a-zA-Z0-9_.-]+)*$")
    CONST MAX_KEY_LENGTH = 256
    CONST MAX_VALUE_SIZE = 1_048_576  // 1MB
    CONST INJECTION_PATTERNS = [
        regex("(\\.\\./)+"),           // Path traversal
        regex("';|--|/\\*|\\*/"),      // SQL injection
        regex("<script|javascript:"),   // XSS
        regex("\\$\\{|\\$\\("),        // Command injection
    ]
    
    FUNCTION validate_key(key: string) -> Result:
        // Length check
        IF key.length == 0 OR key.length > MAX_KEY_LENGTH:
            RETURN Error("Key length invalid")
        
        // Format check
        IF NOT KEY_PATTERN.matches(key):
            RETURN Error("Key contains invalid characters")
        
        // Injection check
        FOR pattern IN INJECTION_PATTERNS:
            IF pattern.matches(key):
                RETURN Error("Potential injection detected")
        
        RETURN Ok()
    
    FUNCTION validate_value(value: ConfigValue) -> Result:
        MATCH value:
            CASE String(s):
                IF s.length > MAX_VALUE_SIZE:
                    RETURN Error("Value too large")
                FOR pattern IN INJECTION_PATTERNS:
                    IF pattern.matches(s):
                        RETURN Error("Potential injection in value")
            
            CASE Number(n):
                IF n.is_infinite() OR n.is_nan():
                    RETURN Error("Invalid number")
            
            CASE Object(map):
                IF map.len() > 1000:
                    RETURN Error("Object has too many keys")
                FOR (k, v) IN map:
                    validate_key(k)?
                    validate_value(v)?
            
            CASE Array(arr):
                IF arr.len() > 10000:
                    RETURN Error("Array too large")
                FOR item IN arr:
                    validate_value(item)?
        
        RETURN Ok()
```

## 8. Integration Points

```pseudocode
MODULE SecureConfigStore:
    store: ThreadSafeStore
    loader: SecureFileLoader
    limiter: RateLimiter
    sanitizer: ErrorSanitizer
    
    ASYNC FUNCTION get(client_id: string, key: string) -> Result<ConfigValue>:
        // Rate limiting
        limiter.check_limit(client_id)
            .map_err(|e| sanitizer.sanitize(e))?
        
        // Get from store
        store.get(key).await
            .map_err(|e| sanitizer.sanitize(e))
    
    ASYNC FUNCTION set(client_id: string, key: string, value: ConfigValue) -> Result:
        // Rate limiting
        limiter.check_limit(client_id)
            .map_err(|e| sanitizer.sanitize(e))?
        
        // Store with all validations
        store.set(key, value).await
            .map_err(|e| sanitizer.sanitize(e))
    
    FUNCTION load_from_file(client_id: string, path: string) -> Result<ConfigValue>:
        // Rate limiting
        limiter.check_limit(client_id)
            .map_err(|e| sanitizer.sanitize(e))?
        
        // Secure file loading
        content = loader.load_file(path)
            .map_err(|e| sanitizer.sanitize(e))?
        
        // Safe parsing
        SafeJsonParser::parse_safe(content)
            .map_err(|e| sanitizer.sanitize(e))
```

## Test Helpers

```pseudocode
MODULE TestHelpers:
    FUNCTION create_nested_json(depth: int) -> string:
        IF depth <= 0:
            RETURN '{"value": "leaf"}'
        ELSE:
            RETURN '{"nested": ' + create_nested_json(depth - 1) + '}'
    
    FUNCTION generate_large_json(size_mb: int) -> string:
        value = "x".repeat(size_mb * 1_000_000)
        RETURN '{"data": "' + value + '"}'
    
    FUNCTION simulate_concurrent_access(store: Store, operations: int):
        handles = []
        FOR i IN 0..operations:
            handle = spawn_async:
                IF i % 2 == 0:
                    store.set("key" + i, "value" + i)
                ELSE:
                    store.get("key" + (i-1))
            handles.push(handle)
        
        wait_all(handles)
```