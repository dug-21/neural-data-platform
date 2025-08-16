# Memory Optimization: Multilayer Ensemble System

## Overview

This document outlines comprehensive memory optimization strategies for the multilayer ensemble neural system, targeting <100MB per symbol while supporting 100+ symbols simultaneously.

## Memory Architecture

### Current Memory Usage Analysis
```ascii
Current Memory Footprint (Per Symbol):
┌─────────────────────────────────────────────────────────────┐
│                    Memory Usage Breakdown                  │
├─────────────────────────────────────────────────────────────┤
│ Component                │ Current (MB) │ Target (MB)      │
├─────────────────────────────────────────────────────────────┤
│ Symbol Models            │ 45-60        │ 20-30           │
│ Feature Storage          │ 25-35        │ 10-15           │
│ Sector Pools             │ 30-40        │ 15-25           │
│ Prediction Cache         │ 15-20        │ 5-10            │
│ Metadata & Config        │ 10-15        │ 5-10            │
│ Specialization Models    │ 20-25        │ 10-15           │
├─────────────────────────────────────────────────────────────┤
│ Total Per Symbol         │ 145-195      │ 65-105          │
│ Target Achievement       │ ❌ Exceeds    │ ✅ Within Limit │
└─────────────────────────────────────────────────────────────┘
```

### Target Memory Architecture
```ascii
Optimized Memory Layout:
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Memory Optimization Strategy                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│ Shared Memory Pools:                                                        │
│ ┌─────────────────────────────────────────────────────────────────────┐    │
│ │ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐    │    │
│ │ │ Sector Pool │ │ Feature     │ │ Model       │ │ Metadata    │    │    │
│ │ │ (Tech: 50MB)│ │ Cache       │ │ Templates   │ │ Cache       │    │    │
│ │ │             │ │ (Global:    │ │ (Shared:    │ │ (Global:    │    │    │
│ │ │ Shared by:  │ │ 30MB)       │ │ 40MB)       │ │ 10MB)       │    │    │
│ │ │ NVDA,AAPL,  │ │             │ │             │ │             │    │    │
│ │ │ GOOGL,MSFT  │ │             │ │             │ │             │    │    │
│ │ └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘    │    │
│ └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│ Per-Symbol Memory (Optimized):                                             │
│ ┌─────────────────────────────────────────────────────────────────────┐    │
│ │ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐    │    │
│ │ │ Symbol-     │ │ Recent      │ │ Prediction  │ │ Performance │    │    │
│ │ │ Specific    │ │ Data Buffer │ │ Cache       │ │ Metrics     │    │    │
│ │ │ Weights     │ │ (Rolling:   │ │ (LRU:       │ │ (Compact:   │    │    │
│ │ │ (5MB)       │ │ 8MB)        │ │ 3MB)        │ │ 2MB)        │    │    │
│ │ └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘    │    │
│ └─────────────────────────────────────────────────────────────────────┘    │
│ Total Per Symbol: ~18MB + Shared Pool Access                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Memory Optimization Strategies

### 1. Shared Model Pools
```rust
/// Optimized cluster model pool with memory management
pub struct OptimizedClusterModelPool {
    /// Shared models with memory-mapped access
    shared_models: Arc<MemoryMappedModelStorage>,
    
    /// LRU cache for frequently accessed model components
    active_components: Arc<LruCache<ModelComponentKey, ModelComponent>>,
    
    /// Memory allocator with custom strategy
    allocator: CustomNeuralAllocator,
    
    /// Memory usage tracking
    memory_tracker: Arc<MemoryTracker>,
    
    /// Lazy loading configuration
    lazy_config: LazyLoadingConfig,
}

impl OptimizedClusterModelPool {
    /// Create pool with memory constraints
    pub async fn new_with_constraints(
        sector_id: String,
        memory_limit_mb: f64,
        max_active_models: usize,
    ) -> Result<Self> {
        let allocator = CustomNeuralAllocator::new(
            (memory_limit_mb * 1024.0 * 1024.0) as usize
        );
        
        let active_components = Arc::new(LruCache::new(max_active_models));
        let memory_tracker = Arc::new(MemoryTracker::new(memory_limit_mb));
        
        Ok(Self {
            shared_models: Arc::new(MemoryMappedModelStorage::new().await?),
            active_components,
            allocator,
            memory_tracker,
            lazy_config: LazyLoadingConfig::optimized(),
        })
    }
    
    /// Add model with memory optimization
    pub async fn add_optimized_model(
        &self,
        model_type: &str,
        model_data: &[u8],
    ) -> Result<()> {
        // Check memory constraints
        let estimated_size = model_data.len();
        if !self.memory_tracker.can_allocate(estimated_size).await? {
            self.evict_least_used_components().await?;
        }
        
        // Compress model data
        let compressed_data = self.compress_model_data(model_data).await?;
        
        // Store in memory-mapped file
        self.shared_models.store_compressed(model_type, compressed_data).await?;
        
        // Update memory tracking
        self.memory_tracker.track_allocation(model_type, estimated_size).await?;
        
        Ok(())
    }
    
    /// Get model with lazy loading
    pub async fn get_model_lazy(
        &self,
        model_type: &str,
    ) -> Result<LazyModelHandle> {
        // Check if already in active cache
        if let Some(component) = self.active_components.get(&ModelComponentKey::new(model_type)) {
            return Ok(LazyModelHandle::Active(component.clone()));
        }
        
        // Return lazy handle that loads on first use
        Ok(LazyModelHandle::Lazy(LazyModelLoader {
            model_type: model_type.to_string(),
            storage: Arc::clone(&self.shared_models),
            cache: Arc::clone(&self.active_components),
            allocator: self.allocator.clone(),
        }))
    }
    
    /// Compress model data using neural-specific compression
    async fn compress_model_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Use quantization for neural network weights
        let quantized = self.quantize_weights(data).await?;
        
        // Apply lossless compression
        let compressed = self.lossless_compress(&quantized).await?;
        
        Ok(compressed)
    }
    
    /// Quantize neural network weights to reduce precision
    async fn quantize_weights(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Convert f32 weights to i8 (75% size reduction)
        let weights: &[f32] = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const f32,
                data.len() / 4
            )
        };
        
        // Calculate quantization parameters
        let min_val = weights.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_val = weights.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let scale = (max_val - min_val) / 255.0;
        
        let mut quantized = Vec::with_capacity(weights.len() + 8); // +8 for scale/offset
        
        // Store quantization parameters
        quantized.extend_from_slice(&scale.to_le_bytes());
        quantized.extend_from_slice(&min_val.to_le_bytes());
        
        // Quantize weights
        for &weight in weights {
            let quantized_val = ((weight - min_val) / scale) as u8;
            quantized.push(quantized_val);
        }
        
        Ok(quantized)
    }
    
    /// Evict least used components to free memory
    async fn evict_least_used_components(&self) -> Result<()> {
        let eviction_count = self.active_components.len() / 4; // Evict 25%
        
        for _ in 0..eviction_count {
            if let Some((key, _)) = self.active_components.pop_lru() {
                self.memory_tracker.track_deallocation(&key.model_type).await?;
            }
        }
        
        Ok(())
    }
}
```

### 2. Feature Compression and Sharing
```rust
/// Compressed feature storage with sharing
pub struct CompressedFeatureStore {
    /// Compressed feature data
    compressed_features: Arc<DashMap<FeatureKey, CompressedFeatureData>>,
    
    /// Feature index for fast lookup
    feature_index: Arc<FeatureIndex>,
    
    /// Memory allocator for features
    feature_allocator: FeatureAllocator,
    
    /// Compression statistics
    compression_stats: Arc<RwLock<CompressionStats>>,
}

impl CompressedFeatureStore {
    /// Store features with compression
    pub async fn store_compressed_features(
        &self,
        symbol: &str,
        features: &[f64],
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let key = FeatureKey::new(symbol, timestamp);
        
        // Apply feature-specific compression
        let compressed = self.compress_features(features).await?;
        
        // Store with automatic expiration
        let ttl = chrono::Duration::hours(24);
        let data = CompressedFeatureData {
            compressed_data: compressed,
            original_size: features.len() * 8, // f64 = 8 bytes
            compressed_size: compressed.len(),
            timestamp,
            ttl,
        };
        
        self.compressed_features.insert(key.clone(), data);
        self.feature_index.add_entry(key, timestamp).await?;
        
        // Update compression statistics
        let mut stats = self.compression_stats.write().await;
        stats.update_compression_ratio(
            data.original_size as f64 / data.compressed_size as f64
        );
        
        Ok(())
    }
    
    /// Retrieve and decompress features
    pub async fn get_features(
        &self,
        symbol: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<Vec<f64>>> {
        let key = FeatureKey::new(symbol, timestamp);
        
        if let Some(compressed_data) = self.compressed_features.get(&key) {
            let features = self.decompress_features(&compressed_data.compressed_data).await?;
            Ok(Some(features))
        } else {
            Ok(None)
        }
    }
    
    /// Compress features using domain-specific algorithms
    async fn compress_features(&self, features: &[f64]) -> Result<Vec<u8>> {
        // Use delta encoding for time-series features
        let deltas = self.delta_encode(features)?;
        
        // Quantize to appropriate precision
        let quantized = self.adaptive_quantize(&deltas)?;
        
        // Apply entropy coding
        let compressed = self.entropy_encode(&quantized)?;
        
        Ok(compressed)
    }
    
    /// Delta encoding for time-series data
    fn delta_encode(&self, features: &[f64]) -> Result<Vec<f64>> {
        if features.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut deltas = Vec::with_capacity(features.len());
        deltas.push(features[0]); // First value as-is
        
        for i in 1..features.len() {
            deltas.push(features[i] - features[i-1]);
        }
        
        Ok(deltas)
    }
    
    /// Adaptive quantization based on feature statistics
    fn adaptive_quantize(&self, deltas: &[f64]) -> Result<Vec<i16>> {
        let max_abs = deltas.iter()
            .map(|x| x.abs())
            .fold(0.0, f64::max);
        
        let scale = max_abs / (i16::MAX as f64);
        
        let quantized: Vec<i16> = deltas.iter()
            .map(|&x| (x / scale) as i16)
            .collect();
        
        Ok(quantized)
    }
    
    /// Entropy encoding for final compression
    fn entropy_encode(&self, quantized: &[i16]) -> Result<Vec<u8>> {
        // Use run-length encoding for sparse data
        let rle_encoded = self.run_length_encode(quantized)?;
        
        // Apply zstd compression
        use zstd::stream::encode_all;
        let compressed = encode_all(&rle_encoded[..], 3)?; // Compression level 3
        
        Ok(compressed)
    }
    
    /// Run-length encoding for sparse integer data
    fn run_length_encode(&self, data: &[i16]) -> Result<Vec<u8>> {
        let mut encoded = Vec::new();
        
        if data.is_empty() {
            return Ok(encoded);
        }
        
        let mut current_val = data[0];
        let mut count = 1u16;
        
        for &val in &data[1..] {
            if val == current_val && count < u16::MAX {
                count += 1;
            } else {
                // Encode (value, count) pair
                encoded.extend_from_slice(&current_val.to_le_bytes());
                encoded.extend_from_slice(&count.to_le_bytes());
                current_val = val;
                count = 1;
            }
        }
        
        // Encode final pair
        encoded.extend_from_slice(&current_val.to_le_bytes());
        encoded.extend_from_slice(&count.to_le_bytes());
        
        Ok(encoded)
    }
}

/// Compression statistics for monitoring
#[derive(Debug, Default)]
pub struct CompressionStats {
    pub total_original_size: usize,
    pub total_compressed_size: usize,
    pub average_compression_ratio: f64,
    pub compression_operations: usize,
}

impl CompressionStats {
    pub fn update_compression_ratio(&mut self, ratio: f64) {
        self.compression_operations += 1;
        self.average_compression_ratio = (self.average_compression_ratio * 
            (self.compression_operations - 1) as f64 + ratio) / 
            self.compression_operations as f64;
    }
    
    pub fn overall_compression_ratio(&self) -> f64 {
        if self.total_compressed_size > 0 {
            self.total_original_size as f64 / self.total_compressed_size as f64
        } else {
            1.0
        }
    }
}
```

### 3. Lazy Loading and Eviction
```rust
/// Lazy loading model handle
pub enum LazyModelHandle {
    Active(Arc<ModelComponent>),
    Lazy(LazyModelLoader),
}

impl LazyModelHandle {
    /// Get model component, loading if necessary
    pub async fn get(&self) -> Result<Arc<ModelComponent>> {
        match self {
            LazyModelHandle::Active(component) => Ok(Arc::clone(component)),
            LazyModelHandle::Lazy(loader) => loader.load().await,
        }
    }
    
    /// Check if model is currently loaded
    pub fn is_loaded(&self) -> bool {
        matches!(self, LazyModelHandle::Active(_))
    }
}

/// Lazy model loader
pub struct LazyModelLoader {
    model_type: String,
    storage: Arc<MemoryMappedModelStorage>,
    cache: Arc<LruCache<ModelComponentKey, ModelComponent>>,
    allocator: CustomNeuralAllocator,
}

impl LazyModelLoader {
    /// Load model component on demand
    pub async fn load(&self) -> Result<Arc<ModelComponent>> {
        let key = ModelComponentKey::new(&self.model_type);
        
        // Check cache first
        if let Some(component) = self.cache.get(&key) {
            return Ok(component);
        }
        
        // Load from storage
        let compressed_data = self.storage.load_compressed(&self.model_type).await?;
        
        // Decompress
        let decompressed = self.decompress_model_data(&compressed_data).await?;
        
        // Create model component
        let component = Arc::new(ModelComponent::from_data(decompressed)?);
        
        // Add to cache
        self.cache.put(key, Arc::clone(&component));
        
        Ok(component)
    }
    
    /// Decompress model data
    async fn decompress_model_data(&self, compressed: &[u8]) -> Result<Vec<u8>> {
        // Reverse the compression process
        let lossless_decompressed = self.lossless_decompress(compressed).await?;
        let dequantized = self.dequantize_weights(&lossless_decompressed).await?;
        
        Ok(dequantized)
    }
    
    /// Dequantize neural network weights
    async fn dequantize_weights(&self, quantized: &[u8]) -> Result<Vec<u8>> {
        if quantized.len() < 8 {
            return Err(anyhow!("Invalid quantized data"));
        }
        
        // Extract quantization parameters
        let scale = f32::from_le_bytes([quantized[0], quantized[1], quantized[2], quantized[3]]);
        let min_val = f32::from_le_bytes([quantized[4], quantized[5], quantized[6], quantized[7]]);
        
        let quantized_weights = &quantized[8..];
        
        // Dequantize weights
        let mut weights = Vec::with_capacity(quantized_weights.len() * 4);
        for &quantized_val in quantized_weights {
            let weight = min_val + (quantized_val as f32) * scale;
            weights.extend_from_slice(&weight.to_le_bytes());
        }
        
        Ok(weights)
    }
}
```

### 4. Memory-Mapped Storage
```rust
/// Memory-mapped model storage for efficient access
pub struct MemoryMappedModelStorage {
    /// Base directory for memory-mapped files
    base_dir: PathBuf,
    
    /// Memory-mapped file handles
    mmap_files: Arc<DashMap<String, memmap2::Mmap>>,
    
    /// File metadata
    file_metadata: Arc<DashMap<String, FileMetadata>>,
}

impl MemoryMappedModelStorage {
    /// Create new memory-mapped storage
    pub async fn new() -> Result<Self> {
        let base_dir = PathBuf::from("/tmp/neural-trader/mmap");
        tokio::fs::create_dir_all(&base_dir).await?;
        
        Ok(Self {
            base_dir,
            mmap_files: Arc::new(DashMap::new()),
            file_metadata: Arc::new(DashMap::new()),
        })
    }
    
    /// Store compressed model data
    pub async fn store_compressed(
        &self,
        model_type: &str,
        compressed_data: Vec<u8>,
    ) -> Result<()> {
        let file_path = self.base_dir.join(format!("{}.mmap", model_type));
        
        // Write to file
        tokio::fs::write(&file_path, &compressed_data).await?;
        
        // Create memory mapping
        let file = std::fs::File::open(&file_path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        
        // Store metadata
        let metadata = FileMetadata {
            file_size: compressed_data.len(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };
        
        self.mmap_files.insert(model_type.to_string(), mmap);
        self.file_metadata.insert(model_type.to_string(), metadata);
        
        Ok(())
    }
    
    /// Load compressed model data
    pub async fn load_compressed(&self, model_type: &str) -> Result<Vec<u8>> {
        if let Some(mmap) = self.mmap_files.get(model_type) {
            // Update access time
            if let Some(mut metadata) = self.file_metadata.get_mut(model_type) {
                metadata.last_accessed = Utc::now();
            }
            
            Ok(mmap.to_vec())
        } else {
            // Load from file if not in memory
            let file_path = self.base_dir.join(format!("{}.mmap", model_type));
            if file_path.exists() {
                let data = tokio::fs::read(&file_path).await?;
                
                // Create memory mapping
                let file = std::fs::File::open(&file_path)?;
                let mmap = unsafe { memmap2::Mmap::map(&file)? };
                
                self.mmap_files.insert(model_type.to_string(), mmap);
                
                Ok(data)
            } else {
                Err(anyhow!("Model not found: {}", model_type))
            }
        }
    }
}

#[derive(Debug, Clone)]
struct FileMetadata {
    file_size: usize,
    created_at: DateTime<Utc>,
    last_accessed: DateTime<Utc>,
}
```

### 5. Custom Neural Allocator
```rust
/// Custom memory allocator optimized for neural networks
pub struct CustomNeuralAllocator {
    /// Memory pool for different size classes
    size_pools: Vec<MemoryPool>,
    
    /// Large allocation tracker
    large_allocations: Arc<DashMap<usize, LargeAllocation>>,
    
    /// Total memory limit
    memory_limit: usize,
    
    /// Current memory usage
    current_usage: Arc<AtomicUsize>,
}

impl CustomNeuralAllocator {
    /// Create allocator with memory limit
    pub fn new(memory_limit_bytes: usize) -> Self {
        // Create size pools for common neural network sizes
        let size_pools = vec![
            MemoryPool::new(64),     // Small weights
            MemoryPool::new(256),    // Medium weights
            MemoryPool::new(1024),   // Large weights
            MemoryPool::new(4096),   // Very large weights
        ];
        
        Self {
            size_pools,
            large_allocations: Arc::new(DashMap::new()),
            memory_limit: memory_limit_bytes,
            current_usage: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    /// Allocate memory with size class optimization
    pub fn allocate(&self, size: usize) -> Result<*mut u8> {
        if self.current_usage.load(Ordering::Acquire) + size > self.memory_limit {
            return Err(anyhow!("Memory limit exceeded"));
        }
        
        let ptr = if size <= 4096 {
            // Use size pools for small allocations
            self.allocate_from_pool(size)?
        } else {
            // Direct allocation for large blocks
            self.allocate_large(size)?
        };
        
        self.current_usage.fetch_add(size, Ordering::AcqRel);
        Ok(ptr)
    }
    
    /// Deallocate memory
    pub fn deallocate(&self, ptr: *mut u8, size: usize) {
        if size <= 4096 {
            self.deallocate_to_pool(ptr, size);
        } else {
            self.deallocate_large(ptr, size);
        }
        
        self.current_usage.fetch_sub(size, Ordering::AcqRel);
    }
    
    /// Get current memory usage
    pub fn current_usage(&self) -> usize {
        self.current_usage.load(Ordering::Acquire)
    }
    
    /// Get memory utilization percentage
    pub fn utilization_percentage(&self) -> f64 {
        (self.current_usage() as f64 / self.memory_limit as f64) * 100.0
    }
    
    fn allocate_from_pool(&self, size: usize) -> Result<*mut u8> {
        for pool in &self.size_pools {
            if pool.block_size >= size {
                return pool.allocate();
            }
        }
        
        // Fallback to system allocator
        self.allocate_large(size)
    }
    
    fn allocate_large(&self, size: usize) -> Result<*mut u8> {
        let layout = std::alloc::Layout::from_size_align(size, 8)
            .map_err(|e| anyhow!("Invalid layout: {}", e))?;
        
        let ptr = unsafe { std::alloc::alloc(layout) };
        
        if ptr.is_null() {
            Err(anyhow!("Allocation failed"))
        } else {
            let allocation = LargeAllocation {
                size,
                layout,
                allocated_at: Utc::now(),
            };
            self.large_allocations.insert(ptr as usize, allocation);
            Ok(ptr)
        }
    }
    
    fn deallocate_to_pool(&self, ptr: *mut u8, size: usize) {
        for pool in &self.size_pools {
            if pool.block_size >= size {
                pool.deallocate(ptr);
                return;
            }
        }
        
        // Fallback to system deallocation
        self.deallocate_large(ptr, size);
    }
    
    fn deallocate_large(&self, ptr: *mut u8, _size: usize) {
        if let Some((_, allocation)) = self.large_allocations.remove(&(ptr as usize)) {
            unsafe {
                std::alloc::dealloc(ptr, allocation.layout);
            }
        }
    }
}

/// Memory pool for fixed-size allocations
struct MemoryPool {
    block_size: usize,
    free_blocks: Arc<Mutex<Vec<*mut u8>>>,
    allocated_blocks: Arc<AtomicUsize>,
}

impl MemoryPool {
    fn new(block_size: usize) -> Self {
        Self {
            block_size,
            free_blocks: Arc::new(Mutex::new(Vec::new())),
            allocated_blocks: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    fn allocate(&self) -> Result<*mut u8> {
        let mut free_blocks = self.free_blocks.lock().unwrap();
        
        if let Some(ptr) = free_blocks.pop() {
            self.allocated_blocks.fetch_add(1, Ordering::AcqRel);
            Ok(ptr)
        } else {
            drop(free_blocks);
            
            let layout = std::alloc::Layout::from_size_align(self.block_size, 8)
                .map_err(|e| anyhow!("Invalid layout: {}", e))?;
            
            let ptr = unsafe { std::alloc::alloc(layout) };
            
            if ptr.is_null() {
                Err(anyhow!("Pool allocation failed"))
            } else {
                self.allocated_blocks.fetch_add(1, Ordering::AcqRel);
                Ok(ptr)
            }
        }
    }
    
    fn deallocate(&self, ptr: *mut u8) {
        let mut free_blocks = self.free_blocks.lock().unwrap();
        free_blocks.push(ptr);
        self.allocated_blocks.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct LargeAllocation {
    size: usize,
    layout: std::alloc::Layout,
    allocated_at: DateTime<Utc>,
}
```

## Memory Monitoring and Metrics

### Memory Usage Tracking
```rust
/// Comprehensive memory tracking system
pub struct MemoryTracker {
    /// Per-component memory usage
    component_usage: Arc<DashMap<String, ComponentMemoryUsage>>,
    
    /// Memory limit
    memory_limit_bytes: usize,
    
    /// Alert thresholds
    warning_threshold: f64,  // 0.8 = 80%
    critical_threshold: f64, // 0.95 = 95%
    
    /// Memory usage history for trend analysis
    usage_history: Arc<RwLock<VecDeque<MemorySnapshot>>>,
}

impl MemoryTracker {
    pub async fn track_allocation(&self, component: &str, size: usize) -> Result<()> {
        let mut usage = self.component_usage
            .entry(component.to_string())
            .or_insert(ComponentMemoryUsage::default());
        
        usage.current_usage += size;
        usage.peak_usage = usage.peak_usage.max(usage.current_usage);
        usage.allocation_count += 1;
        
        // Check thresholds
        let total_usage = self.get_total_usage().await;
        let utilization = total_usage as f64 / self.memory_limit_bytes as f64;
        
        if utilization > self.critical_threshold {
            warn!("CRITICAL: Memory usage at {:.1}%", utilization * 100.0);
        } else if utilization > self.warning_threshold {
            warn!("WARNING: Memory usage at {:.1}%", utilization * 100.0);
        }
        
        Ok(())
    }
    
    pub async fn get_memory_report(&self) -> MemoryReport {
        let total_usage = self.get_total_usage().await;
        let utilization = total_usage as f64 / self.memory_limit_bytes as f64;
        
        let component_breakdown: HashMap<String, ComponentMemoryUsage> = 
            self.component_usage.iter()
                .map(|entry| (entry.key().clone(), entry.value().clone()))
                .collect();
        
        MemoryReport {
            total_usage_bytes: total_usage,
            memory_limit_bytes: self.memory_limit_bytes,
            utilization_percentage: utilization * 100.0,
            component_breakdown,
            status: if utilization > self.critical_threshold {
                MemoryStatus::Critical
            } else if utilization > self.warning_threshold {
                MemoryStatus::Warning
            } else {
                MemoryStatus::Normal
            },
        }
    }
    
    async fn get_total_usage(&self) -> usize {
        self.component_usage.iter()
            .map(|entry| entry.value().current_usage)
            .sum()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ComponentMemoryUsage {
    pub current_usage: usize,
    pub peak_usage: usize,
    pub allocation_count: usize,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MemoryReport {
    pub total_usage_bytes: usize,
    pub memory_limit_bytes: usize,
    pub utilization_percentage: f64,
    pub component_breakdown: HashMap<String, ComponentMemoryUsage>,
    pub status: MemoryStatus,
}

#[derive(Debug, Clone)]
pub enum MemoryStatus {
    Normal,
    Warning,
    Critical,
}
```

This comprehensive memory optimization strategy ensures the multilayer ensemble system operates efficiently within the <100MB per symbol target while maintaining high performance and scalability.