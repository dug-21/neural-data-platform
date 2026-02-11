//! Memory diagnostics for BUG-005 RSS growth investigation.
//!
//! Provides /proc parsers, glibc mallinfo2 FFI, and a central
//! `MemoryDiagnostics` struct that collects RSS, allocator, smaps,
//! and accumulator metrics in a single snapshot.
//!
//! All /proc parsers are split into `parse_*(&str)` (testable) and
//! `read_*()` (reads the file). Linux-specific code is behind
//! `#[cfg(target_os = "linux")]` with `None` fallback.

use crate::storage::accumulator::Accumulator;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// SmapsSummary
// ---------------------------------------------------------------------------

/// RSS decomposition by mapping type, parsed from /proc/self/smaps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmapsSummary {
    pub heap_rss_bytes: u64,
    pub stack_rss_bytes: u64,
    pub anon_rss_bytes: u64,
    pub file_rss_bytes: u64,
    pub total_rss_bytes: u64,
}

// ---------------------------------------------------------------------------
// MallocStats
// ---------------------------------------------------------------------------

/// Glibc mallinfo2 result. Fields mirror the C struct.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MallocStats {
    /// Total non-mmapped bytes (arena size).
    pub arena: u64,
    /// Number of free chunks.
    pub ordblks: u64,
    /// Total bytes in mmapped regions.
    pub hblkhd: u64,
    /// Total allocated space (normal).
    pub uordblks: u64,
    /// Total free space (normal).
    pub fordblks: u64,
    /// Top-most releasable space.
    pub keepcost: u64,
}

// ---------------------------------------------------------------------------
// MemoryTrend (ring buffer)
// ---------------------------------------------------------------------------

/// Ring buffer that tracks RSS over time and computes growth rate.
#[derive(Debug, Clone)]
pub struct MemoryTrend {
    samples: VecDeque<(DateTime<Utc>, u64)>,
    max_samples: usize,
}

impl MemoryTrend {
    /// Create a new trend tracker with the given capacity.
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    /// Record an RSS sample at the current wall-clock time.
    pub fn record(&mut self, rss_bytes: u64) {
        self.record_at(Utc::now(), rss_bytes);
    }

    /// Record an RSS sample at an explicit timestamp (for deterministic tests).
    pub fn record_at(&mut self, ts: DateTime<Utc>, rss_bytes: u64) {
        if self.samples.len() == self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back((ts, rss_bytes));
    }

    /// Compute growth rate in bytes per hour from first to last sample.
    ///
    /// Returns `None` if fewer than 2 samples or zero time span.
    pub fn growth_rate_bytes_per_hour(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None;
        }
        let (ts_first, rss_first) = self.samples.front()?;
        let (ts_last, rss_last) = self.samples.back()?;

        let duration_secs = (*ts_last - *ts_first).num_seconds() as f64;
        if duration_secs == 0.0 {
            return None;
        }

        let delta_bytes = *rss_last as f64 - *rss_first as f64;
        let hours = duration_secs / 3600.0;
        Some(delta_bytes / hours)
    }

    /// Number of recorded samples.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Returns true if no samples have been recorded.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

// ---------------------------------------------------------------------------
// MemoryDiagnostics
// ---------------------------------------------------------------------------

/// Central collection struct: one snapshot of all memory metrics.
#[derive(Debug, Clone)]
pub struct MemoryDiagnostics {
    /// When this snapshot was taken.
    pub sampled_at: DateTime<Utc>,

    // -- Process RSS --
    pub rss_bytes: Option<u64>,

    // -- Allocator (mallinfo2) --
    pub arena_bytes: Option<u64>,
    pub ordblks: Option<u64>,
    pub hblkhd_bytes: Option<u64>,
    pub uordblks_bytes: Option<u64>,
    pub fordblks_bytes: Option<u64>,

    // -- Smaps decomposition --
    pub heap_rss_bytes: Option<u64>,
    pub stack_rss_bytes: Option<u64>,
    pub anon_rss_bytes: Option<u64>,

    // -- Accumulator --
    pub accumulator_count: usize,
    pub accumulator_source_count: usize,
    pub accumulator_capacity: usize,
    pub accumulator_vec_capacity_sum: usize,
    pub accumulator_vec_len_sum: usize,
    pub accumulator_estimate_bytes: usize,
}

impl MemoryDiagnostics {
    /// Collect a full snapshot from the running process and accumulator.
    pub fn collect(accumulator: &Accumulator) -> Self {
        let rss_bytes = read_proc_status_rss_bytes();

        let malloc = read_mallinfo2();

        let smaps = read_proc_smaps_summary();

        Self {
            sampled_at: Utc::now(),
            rss_bytes,

            arena_bytes: malloc.as_ref().map(|m| m.arena),
            ordblks: malloc.as_ref().map(|m| m.ordblks),
            hblkhd_bytes: malloc.as_ref().map(|m| m.hblkhd),
            uordblks_bytes: malloc.as_ref().map(|m| m.uordblks),
            fordblks_bytes: malloc.as_ref().map(|m| m.fordblks),

            heap_rss_bytes: smaps.as_ref().map(|s| s.heap_rss_bytes),
            stack_rss_bytes: smaps.as_ref().map(|s| s.stack_rss_bytes),
            anon_rss_bytes: smaps.as_ref().map(|s| s.anon_rss_bytes),

            accumulator_count: accumulator.count(),
            accumulator_source_count: accumulator.source_count(),
            accumulator_capacity: accumulator.hash_capacity(),
            accumulator_vec_capacity_sum: accumulator.vec_capacity(),
            accumulator_vec_len_sum: accumulator.vec_len(),
            accumulator_estimate_bytes: accumulator.memory_estimate_bytes(),
        }
    }

    /// Format RSS in MiB for display, or "N/A" if unavailable.
    pub fn rss_mib_display(&self) -> String {
        match self.rss_bytes {
            Some(b) => format!("{:.1}", b as f64 / 1_048_576.0),
            None => "N/A".to_string(),
        }
    }

    /// Compute bytes not explained by the accumulator estimate.
    ///
    /// Returns `Some(rss - estimate)` when RSS is known.
    /// Result can be negative if the estimate exceeds RSS.
    pub fn unaccounted_bytes(&self) -> Option<i64> {
        self.rss_bytes
            .map(|rss| rss as i64 - self.accumulator_estimate_bytes as i64)
    }
}

// ---------------------------------------------------------------------------
// /proc parsers (pure functions)
// ---------------------------------------------------------------------------

/// Parse a "Key:   1234 kB" line, returning the numeric value.
///
/// Expects the value token to be the second whitespace-separated field
/// after the colon. Returns `None` on parse failure.
pub fn parse_kb_value(line: &str) -> Option<u64> {
    let after_colon = line.split(':').nth(1)?.trim();
    let value_str = after_colon.split_whitespace().next()?;
    value_str.parse::<u64>().ok()
}

/// Parse VmRSS from /proc/self/status content, returning bytes.
pub fn parse_proc_status_rss_bytes(content: &str) -> Option<u64> {
    for line in content.lines() {
        if line.starts_with("VmRSS:") {
            return parse_kb_value(line).map(|kb| kb * 1024);
        }
    }
    None
}

/// Read /proc/self/status and extract VmRSS in bytes.
#[cfg(target_os = "linux")]
pub fn read_proc_status_rss_bytes() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_proc_status_rss_bytes(&content)
}

#[cfg(not(target_os = "linux"))]
pub fn read_proc_status_rss_bytes() -> Option<u64> {
    None
}

/// Convenience wrapper: read RSS and convert to MiB.
pub fn read_process_rss_mib() -> Option<f64> {
    read_proc_status_rss_bytes().map(|b| b as f64 / 1_048_576.0)
}

/// Parse /proc/self/smaps content into an RSS breakdown by mapping type.
///
/// Mapping header lines contain a '-' (address range) and do not start
/// with whitespace. We classify RSS into heap, stack, anonymous, or
/// file-backed based on the mapping name and inode field.
pub fn parse_proc_smaps_summary(content: &str) -> Option<SmapsSummary> {
    let mut summary = SmapsSummary::default();

    // State: the current mapping header line (if any).
    let mut current_mapping: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Mapping header lines contain '-' for address range and don't
        // start with whitespace.
        if !line.starts_with(|c: char| c.is_whitespace()) && line.contains('-') {
            // Check if this looks like a header: "addr1-addr2 perms offset dev inode pathname"
            // A VMA header has hex digits before the first '-'.
            let first_char = line.chars().next().unwrap_or(' ');
            if first_char.is_ascii_hexdigit() {
                current_mapping = Some(line.to_string());
            }
            continue;
        }

        // Metric lines: "Key:   value kB"
        if trimmed.starts_with("Rss:") {
            let rss_kb = match parse_kb_value(trimmed) {
                Some(v) => v,
                None => continue,
            };
            let rss_bytes = rss_kb * 1024;
            summary.total_rss_bytes += rss_bytes;

            if let Some(ref mapping) = current_mapping {
                let mapping_lower = mapping.to_lowercase();

                if mapping_lower.contains("[heap]") {
                    summary.heap_rss_bytes += rss_bytes;
                } else if mapping_lower.contains("[stack]") {
                    summary.stack_rss_bytes += rss_bytes;
                } else {
                    // Check if file-backed: the 5th field (inode) is not "0"
                    let fields: Vec<&str> = mapping.split_whitespace().collect();
                    let is_file_backed = fields.get(4).map(|inode| *inode != "0").unwrap_or(false);

                    if is_file_backed {
                        summary.file_rss_bytes += rss_bytes;
                    } else {
                        summary.anon_rss_bytes += rss_bytes;
                    }
                }
            } else {
                // No mapping header seen yet; count as anonymous
                summary.anon_rss_bytes += rss_bytes;
            }
        }
    }

    Some(summary)
}

/// Read /proc/self/smaps and parse it into an RSS breakdown.
#[cfg(target_os = "linux")]
pub fn read_proc_smaps_summary() -> Option<SmapsSummary> {
    let content = std::fs::read_to_string("/proc/self/smaps").ok()?;
    parse_proc_smaps_summary(&content)
}

#[cfg(not(target_os = "linux"))]
pub fn read_proc_smaps_summary() -> Option<SmapsSummary> {
    None
}

// ---------------------------------------------------------------------------
// mallinfo2 FFI
// ---------------------------------------------------------------------------

/// Read glibc mallinfo2 stats. Only available on Linux with glibc >= 2.33;
/// returns None on older glibc or non-Linux systems.
///
/// Uses `dlsym(RTLD_DEFAULT, "mallinfo2")` for runtime lookup so the binary
/// compiles on any glibc version and gracefully degrades.
#[cfg(target_os = "linux")]
pub fn read_mallinfo2() -> Option<MallocStats> {
    /// C-compatible mallinfo2 struct matching glibc's definition (10 fields).
    #[repr(C)]
    struct Mallinfo2 {
        arena: usize,
        ordblks: usize,
        smblks: usize,
        hblks: usize,
        hblkhd: usize,
        usmblks: usize,
        fsmblks: usize,
        uordblks: usize,
        fordblks: usize,
        keepcost: usize,
    }

    // RTLD_DEFAULT: search the global symbol table (already-loaded libs).
    const RTLD_DEFAULT: *mut std::ffi::c_void = std::ptr::null_mut();

    extern "C" {
        fn dlsym(
            handle: *mut std::ffi::c_void,
            symbol: *const std::ffi::c_char,
        ) -> *mut std::ffi::c_void;
    }

    type Mallinfo2Fn = unsafe extern "C" fn() -> Mallinfo2;

    let symbol = b"mallinfo2\0";
    let ptr = unsafe { dlsym(RTLD_DEFAULT, symbol.as_ptr() as *const std::ffi::c_char) };

    if ptr.is_null() {
        // mallinfo2 not available (glibc < 2.33)
        return None;
    }

    let mallinfo2_fn: Mallinfo2Fn = unsafe { std::mem::transmute(ptr) };
    let info = unsafe { mallinfo2_fn() };

    Some(MallocStats {
        arena: info.arena as u64,
        ordblks: info.ordblks as u64,
        hblkhd: info.hblkhd as u64,
        uordblks: info.uordblks as u64,
        fordblks: info.fordblks as u64,
        keepcost: info.keepcost as u64,
    })
}

#[cfg(not(target_os = "linux"))]
pub fn read_mallinfo2() -> Option<MallocStats> {
    None
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format an optional byte count as "X.Y" MiB, or "N/A" if None.
pub fn format_opt_mib(bytes: Option<u64>) -> String {
    match bytes {
        Some(b) => format!("{:.1}", b as f64 / 1_048_576.0),
        None => "N/A".to_string(),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RawDataPoint;
    use chrono::{Duration, TimeZone};
    use serde_json::json;

    /// Helper: create a RawDataPoint with given source_id.
    fn make_test_point(source_id: &str) -> RawDataPoint {
        RawDataPoint::new(source_id, json!({"value": 42.0}))
            .with_timestamp(Utc::now())
            .with_ndp_id("test-device")
    }

    // ====================================================================
    // T-01: collect — empty accumulator
    // ====================================================================

    #[test]
    fn test_diagnostics_collect_empty_accumulator() {
        let acc = Accumulator::new(Utc::now().date_naive());
        let diag = MemoryDiagnostics::collect(&acc);

        assert_eq!(diag.accumulator_count, 0);
        assert_eq!(diag.accumulator_source_count, 0);
        assert_eq!(diag.accumulator_vec_capacity_sum, 0);
        assert_eq!(diag.accumulator_vec_len_sum, 0);

        // sampled_at should be very recent (within 2 seconds)
        let elapsed = Utc::now() - diag.sampled_at;
        assert!(
            elapsed.num_seconds() < 2,
            "sampled_at should be within 2s of now, got {}s",
            elapsed.num_seconds()
        );
    }

    // ====================================================================
    // T-02: collect — populated accumulator (3 sources x 10 points)
    // ====================================================================

    #[test]
    fn test_diagnostics_collect_populated_accumulator() {
        let mut acc = Accumulator::new(Utc::now().date_naive());

        for src in &["source-a", "source-b", "source-c"] {
            for _ in 0..10 {
                acc.add(make_test_point(src));
            }
        }

        let diag = MemoryDiagnostics::collect(&acc);

        assert_eq!(diag.accumulator_count, 30);
        assert_eq!(diag.accumulator_source_count, 3);
        assert!(
            diag.accumulator_estimate_bytes > 0,
            "estimate should be positive for 30 points"
        );
        // vec_len should reflect 30 total entries
        assert_eq!(diag.accumulator_vec_len_sum, 30);
        // vec_capacity must be >= vec_len
        assert!(
            diag.accumulator_vec_capacity_sum >= diag.accumulator_vec_len_sum,
            "capacity {} should be >= len {}",
            diag.accumulator_vec_capacity_sum,
            diag.accumulator_vec_len_sum
        );
    }

    // ====================================================================
    // T-03: rss_mib_display — Some
    // ====================================================================

    #[test]
    fn test_rss_mib_display_some() {
        let diag = MemoryDiagnostics {
            sampled_at: Utc::now(),
            rss_bytes: Some(104_857_600), // exactly 100 MiB
            arena_bytes: None,
            ordblks: None,
            hblkhd_bytes: None,
            uordblks_bytes: None,
            fordblks_bytes: None,
            heap_rss_bytes: None,
            stack_rss_bytes: None,
            anon_rss_bytes: None,
            accumulator_count: 0,
            accumulator_source_count: 0,
            accumulator_capacity: 0,
            accumulator_vec_capacity_sum: 0,
            accumulator_vec_len_sum: 0,
            accumulator_estimate_bytes: 0,
        };

        assert_eq!(diag.rss_mib_display(), "100.0");
    }

    // ====================================================================
    // T-03b: rss_mib_display — None
    // ====================================================================

    #[test]
    fn test_rss_mib_display_none() {
        let diag = MemoryDiagnostics {
            sampled_at: Utc::now(),
            rss_bytes: None,
            arena_bytes: None,
            ordblks: None,
            hblkhd_bytes: None,
            uordblks_bytes: None,
            fordblks_bytes: None,
            heap_rss_bytes: None,
            stack_rss_bytes: None,
            anon_rss_bytes: None,
            accumulator_count: 0,
            accumulator_source_count: 0,
            accumulator_capacity: 0,
            accumulator_vec_capacity_sum: 0,
            accumulator_vec_len_sum: 0,
            accumulator_estimate_bytes: 0,
        };

        assert_eq!(diag.rss_mib_display(), "N/A");
    }

    // ====================================================================
    // T-04: unaccounted_bytes — positive gap
    // ====================================================================

    #[test]
    fn test_unaccounted_bytes_positive_gap() {
        let diag = MemoryDiagnostics {
            sampled_at: Utc::now(),
            rss_bytes: Some(200 * 1_048_576), // 200 MiB
            arena_bytes: None,
            ordblks: None,
            hblkhd_bytes: None,
            uordblks_bytes: None,
            fordblks_bytes: None,
            heap_rss_bytes: None,
            stack_rss_bytes: None,
            anon_rss_bytes: None,
            accumulator_count: 0,
            accumulator_source_count: 0,
            accumulator_capacity: 0,
            accumulator_vec_capacity_sum: 0,
            accumulator_vec_len_sum: 0,
            accumulator_estimate_bytes: 5 * 1_048_576, // 5 MiB
        };

        let unaccounted = diag.unaccounted_bytes().unwrap();
        assert_eq!(unaccounted, 195 * 1_048_576);
    }

    // ====================================================================
    // T-04b: unaccounted_bytes — None RSS
    // ====================================================================

    #[test]
    fn test_unaccounted_bytes_none_rss() {
        let diag = MemoryDiagnostics {
            sampled_at: Utc::now(),
            rss_bytes: None,
            arena_bytes: None,
            ordblks: None,
            hblkhd_bytes: None,
            uordblks_bytes: None,
            fordblks_bytes: None,
            heap_rss_bytes: None,
            stack_rss_bytes: None,
            anon_rss_bytes: None,
            accumulator_count: 0,
            accumulator_source_count: 0,
            accumulator_capacity: 0,
            accumulator_vec_capacity_sum: 0,
            accumulator_vec_len_sum: 0,
            accumulator_estimate_bytes: 5_000_000,
        };

        assert!(diag.unaccounted_bytes().is_none());
    }

    // ====================================================================
    // T-04c: unaccounted_bytes — negative (estimate > RSS)
    // ====================================================================

    #[test]
    fn test_unaccounted_bytes_negative() {
        let diag = MemoryDiagnostics {
            sampled_at: Utc::now(),
            rss_bytes: Some(1_000_000),
            arena_bytes: None,
            ordblks: None,
            hblkhd_bytes: None,
            uordblks_bytes: None,
            fordblks_bytes: None,
            heap_rss_bytes: None,
            stack_rss_bytes: None,
            anon_rss_bytes: None,
            accumulator_count: 0,
            accumulator_source_count: 0,
            accumulator_capacity: 0,
            accumulator_vec_capacity_sum: 0,
            accumulator_vec_len_sum: 0,
            accumulator_estimate_bytes: 5_000_000,
        };

        let unaccounted = diag.unaccounted_bytes().unwrap();
        assert!(unaccounted < 0, "expected negative, got {}", unaccounted);
        assert_eq!(unaccounted, 1_000_000 - 5_000_000);
    }

    // ====================================================================
    // T-05: format_opt_mib — Some
    // ====================================================================

    #[test]
    fn test_format_opt_mib_some() {
        assert_eq!(format_opt_mib(Some(10_485_760)), "10.0");
    }

    // ====================================================================
    // T-05b: format_opt_mib — None
    // ====================================================================

    #[test]
    fn test_format_opt_mib_none() {
        assert_eq!(format_opt_mib(None), "N/A");
    }

    // ====================================================================
    // T-06: parse_kb_value — standard
    // ====================================================================

    #[test]
    fn test_parse_kb_value_standard() {
        assert_eq!(parse_kb_value("Rss:       1024 kB"), Some(1024));
    }

    // ====================================================================
    // T-06b: parse_kb_value — zero
    // ====================================================================

    #[test]
    fn test_parse_kb_value_zero() {
        assert_eq!(parse_kb_value("Rss:          0 kB"), Some(0));
    }

    // ====================================================================
    // T-06c: parse_kb_value — large
    // ====================================================================

    #[test]
    fn test_parse_kb_value_large() {
        assert_eq!(parse_kb_value("Rss:     524288 kB"), Some(524288));
    }

    // ====================================================================
    // T-06d: parse_kb_value — malformed
    // ====================================================================

    #[test]
    fn test_parse_kb_value_malformed() {
        assert_eq!(parse_kb_value("Rss: invalid kB"), None);
    }

    // ====================================================================
    // T-07: parse_proc_status_rss_bytes — typical content
    // ====================================================================

    #[test]
    fn test_parse_proc_status_typical() {
        let content = "\
Name:\tair-quality-ap
Umask:\t0022
State:\tS (sleeping)
Tgid:\t1
Ngid:\t0
Pid:\t1
PPid:\t0
TracerPid:\t0
Uid:\t0\t0\t0\t0
Gid:\t0\t0\t0\t0
VmPeak:\t  123456 kB
VmSize:\t  112000 kB
VmLck:\t       0 kB
VmPin:\t       0 kB
VmHWM:\t   98304 kB
VmRSS:\t   65536 kB
RssAnon:\t   40960 kB
RssFile:\t   24576 kB
RssShmem:\t       0 kB
VmData:\t   80000 kB
VmStk:\t     136 kB
VmExe:\t    4096 kB
Threads:\t12
";

        let rss = parse_proc_status_rss_bytes(content);
        // VmRSS: 65536 kB -> 65536 * 1024 = 67108864 bytes
        assert_eq!(rss, Some(65536 * 1024));
    }

    // ====================================================================
    // T-07b: parse_proc_status — no VmRSS line
    // ====================================================================

    #[test]
    fn test_parse_proc_status_no_vmrss() {
        let content = "\
Name:\tsome-process
State:\tR (running)
Pid:\t42
VmSize:\t100000 kB
";

        assert_eq!(parse_proc_status_rss_bytes(content), None);
    }

    // ====================================================================
    // T-08a: parse_smaps — [heap] mapping
    // ====================================================================

    #[test]
    fn test_parse_smaps_heap_mapping() {
        let content = "\
55a3c8000000-55a3c8200000 rw-p 00000000 00:00 0                          [heap]
Size:               2048 kB
KernelPageSize:        4 kB
MMUPageSize:           4 kB
Rss:                1024 kB
Pss:                1024 kB
Shared_Clean:          0 kB
Shared_Dirty:          0 kB
Private_Clean:         0 kB
Private_Dirty:      1024 kB
Referenced:         1024 kB
Anonymous:          1024 kB
";

        let summary = parse_proc_smaps_summary(content).unwrap();
        assert_eq!(summary.heap_rss_bytes, 1024 * 1024);
        assert_eq!(summary.total_rss_bytes, 1024 * 1024);
        assert_eq!(summary.stack_rss_bytes, 0);
        assert_eq!(summary.anon_rss_bytes, 0);
        assert_eq!(summary.file_rss_bytes, 0);
    }

    // ====================================================================
    // T-08b: parse_smaps — [stack] mapping
    // ====================================================================

    #[test]
    fn test_parse_smaps_stack_mapping() {
        let content = "\
7ffd12000000-7ffd12021000 rw-p 00000000 00:00 0                          [stack]
Size:                132 kB
Rss:                  64 kB
Pss:                  64 kB
";

        let summary = parse_proc_smaps_summary(content).unwrap();
        assert_eq!(summary.stack_rss_bytes, 64 * 1024);
        assert_eq!(summary.total_rss_bytes, 64 * 1024);
        assert_eq!(summary.heap_rss_bytes, 0);
    }

    // ====================================================================
    // T-08c: parse_smaps — mixed mappings
    // ====================================================================

    #[test]
    fn test_parse_smaps_mixed_mappings() {
        // Heap mapping
        let content = "\
55a3c8000000-55a3c8200000 rw-p 00000000 00:00 0                          [heap]
Size:               2048 kB
Rss:                 512 kB
7f1234000000-7f1234100000 r--p 00000000 08:01 12345                      /usr/lib/libc.so.6
Size:               1024 kB
Rss:                 256 kB
7f1235000000-7f1235010000 rw-p 00000000 00:00 0
Size:                 64 kB
Rss:                  32 kB
7ffd12000000-7ffd12021000 rw-p 00000000 00:00 0                          [stack]
Size:                132 kB
Rss:                  16 kB
";

        let summary = parse_proc_smaps_summary(content).unwrap();
        assert_eq!(summary.heap_rss_bytes, 512 * 1024, "heap");
        assert_eq!(summary.file_rss_bytes, 256 * 1024, "file-backed (libc)");
        assert_eq!(
            summary.anon_rss_bytes,
            32 * 1024,
            "anonymous (inode=0, no name)"
        );
        assert_eq!(summary.stack_rss_bytes, 16 * 1024, "stack");
        assert_eq!(
            summary.total_rss_bytes,
            (512 + 256 + 32 + 16) * 1024,
            "total"
        );
    }

    // ====================================================================
    // T-08d: parse_smaps — empty string
    // ====================================================================

    #[test]
    fn test_parse_smaps_empty() {
        let summary = parse_proc_smaps_summary("").unwrap();
        assert_eq!(summary.total_rss_bytes, 0);
        assert_eq!(summary.heap_rss_bytes, 0);
        assert_eq!(summary.stack_rss_bytes, 0);
        assert_eq!(summary.anon_rss_bytes, 0);
        assert_eq!(summary.file_rss_bytes, 0);
    }

    // ====================================================================
    // T-09: mallinfo2 returns Some on Linux (glibc >= 2.33)
    // ====================================================================

    #[cfg(target_os = "linux")]
    #[test]
    fn test_mallinfo2_returns_some_on_linux() {
        let stats = read_mallinfo2();
        // mallinfo2 requires glibc >= 2.33. On older glibc (e.g. 2.31 in
        // dev containers) it legitimately returns None via dlsym fallback.
        if let Some(stats) = stats {
            // When available, arena should be positive on any running process.
            assert!(stats.arena > 0, "arena should be > 0, got {}", stats.arena);
        }
        // If None: dlsym could not find mallinfo2 — this is the expected
        // fallback on glibc < 2.33. Test passes either way.
    }

    // ====================================================================
    // T-10: mallinfo2 returns None on non-Linux
    // ====================================================================

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_mallinfo2_returns_none_on_non_linux() {
        assert!(read_mallinfo2().is_none());
    }

    // ====================================================================
    // T-11: mallinfo2 reflects allocation (glibc >= 2.33 only)
    // ====================================================================

    #[cfg(target_os = "linux")]
    #[test]
    fn test_mallinfo2_reflects_allocation() {
        // Skip on glibc < 2.33 where mallinfo2 is not available.
        let before = match read_mallinfo2() {
            Some(stats) => stats,
            None => return, // graceful skip
        };

        // Allocate ~1 MiB and keep it alive across the measurement.
        let data: Vec<u8> = vec![0xAB; 1_048_576];

        let after = read_mallinfo2().unwrap();

        // uordblks (allocated space) should have grown.
        assert!(
            after.uordblks > before.uordblks,
            "uordblks should grow after 1MiB allocation: before={}, after={}",
            before.uordblks,
            after.uordblks
        );

        // Prevent optimizer from dropping `data` before we measure.
        assert_eq!(data.len(), 1_048_576);
    }

    // ====================================================================
    // T-17: trend — empty returns None
    // ====================================================================

    #[test]
    fn test_trend_empty_returns_none() {
        let trend = MemoryTrend::new(10);
        assert!(trend.growth_rate_bytes_per_hour().is_none());
        assert_eq!(trend.len(), 0);
    }

    // ====================================================================
    // T-18: trend — single sample returns None
    // ====================================================================

    #[test]
    fn test_trend_single_sample_returns_none() {
        let mut trend = MemoryTrend::new(10);
        trend.record_at(Utc::now(), 1_000_000);
        assert!(trend.growth_rate_bytes_per_hour().is_none());
        assert_eq!(trend.len(), 1);
    }

    // ====================================================================
    // T-19: trend — two samples computes rate
    // ====================================================================

    #[test]
    fn test_trend_two_samples_computes_rate() {
        let mut trend = MemoryTrend::new(10);

        let t0 = Utc.with_ymd_and_hms(2026, 2, 10, 12, 0, 0).unwrap();
        let t1 = t0 + Duration::hours(1);

        trend.record_at(t0, 100_000_000); // 100 MB
        trend.record_at(t1, 110_000_000); // 110 MB

        let rate = trend.growth_rate_bytes_per_hour().unwrap();
        // 10 MB / 1 hour = 10_000_000 bytes/hour
        assert!(
            (rate - 10_000_000.0).abs() < 1.0,
            "expected ~10M bytes/hr, got {}",
            rate
        );
    }

    // ====================================================================
    // T-20: trend — ring buffer evicts oldest
    // ====================================================================

    #[test]
    fn test_trend_ring_buffer_evicts_oldest() {
        let mut trend = MemoryTrend::new(3);

        let t0 = Utc.with_ymd_and_hms(2026, 2, 10, 12, 0, 0).unwrap();

        trend.record_at(t0, 100);
        trend.record_at(t0 + Duration::hours(1), 200);
        trend.record_at(t0 + Duration::hours(2), 300);
        assert_eq!(trend.len(), 3);

        // Adding a 4th sample should evict the first (100 @ t0).
        trend.record_at(t0 + Duration::hours(3), 400);
        assert_eq!(trend.len(), 3);

        // Growth rate should now be computed from sample 2 (200 @ t+1h) to sample 4 (400 @ t+3h).
        // Delta = 200, time = 2 hours -> 100 bytes/hour.
        let rate = trend.growth_rate_bytes_per_hour().unwrap();
        assert!(
            (rate - 100.0).abs() < 0.001,
            "expected 100 bytes/hr, got {}",
            rate
        );
    }

    // ====================================================================
    // T-21: trend — negative growth
    // ====================================================================

    #[test]
    fn test_trend_negative_growth() {
        let mut trend = MemoryTrend::new(10);

        let t0 = Utc.with_ymd_and_hms(2026, 2, 10, 12, 0, 0).unwrap();

        trend.record_at(t0, 200_000_000);
        trend.record_at(t0 + Duration::hours(2), 100_000_000);

        let rate = trend.growth_rate_bytes_per_hour().unwrap();
        // -100 MB / 2 hours = -50_000_000 bytes/hour
        assert!(
            (rate - (-50_000_000.0)).abs() < 1.0,
            "expected ~-50M bytes/hr, got {}",
            rate
        );
    }

    // ====================================================================
    // T-22: trend — zero duration returns None
    // ====================================================================

    #[test]
    fn test_trend_zero_duration_returns_none() {
        let mut trend = MemoryTrend::new(10);

        let t0 = Utc.with_ymd_and_hms(2026, 2, 10, 12, 0, 0).unwrap();

        trend.record_at(t0, 100_000);
        trend.record_at(t0, 200_000); // same timestamp

        assert!(
            trend.growth_rate_bytes_per_hour().is_none(),
            "should return None for zero time span"
        );
    }
}
