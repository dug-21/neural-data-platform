use crate::error::{CoreError, CoreResult};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub struct WriteAheadLog {
    path: PathBuf,
    file: File,
}

impl WriteAheadLog {
    pub fn new<P: AsRef<Path>>(path: P) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        Ok(Self { path, file })
    }

    pub fn append(&mut self, entry: &[u8]) -> CoreResult<()> {
        let json_str = std::str::from_utf8(entry)
            .map_err(|e| CoreError::Storage(format!("Invalid UTF-8 in WAL entry: {}", e)))?;
        
        writeln!(self.file, "{}", json_str)?;
        self.file.flush()?;
        
        Ok(())
    }

    pub fn replay(&self) -> CoreResult<Vec<Vec<u8>>> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                entries.push(line.into_bytes());
            }
        }

        Ok(entries)
    }

    pub fn commit(&mut self) -> CoreResult<()> {
        std::fs::remove_file(&self.path)?;
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        
        self.file = file;
        
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn temp_wal_path() -> PathBuf {
        let temp_dir = std::env::temp_dir();
        temp_dir.join(format!("test_wal_{}.log", uuid::Uuid::new_v4()))
    }

    #[test]
    fn test_wal_creation() {
        let path = temp_wal_path();
        let wal = WriteAheadLog::new(&path);
        
        assert!(wal.is_ok());
        assert!(path.exists());
        
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_append_single_entry() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();
        
        let entry = json!({"timestamp": "2024-01-15T10:30:00Z", "value": 42.0});
        let entry_bytes = entry.to_string().into_bytes();
        
        let result = wal.append(&entry_bytes);
        assert!(result.is_ok());
        
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_append_multiple_entries() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();
        
        for i in 0..5 {
            let entry = json!({"id": i, "value": i * 10});
            let entry_bytes = entry.to_string().into_bytes();
            wal.append(&entry_bytes).unwrap();
        }
        
        let replayed = wal.replay().unwrap();
        assert_eq!(replayed.len(), 5);
        
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_replay() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();
        
        let entries = vec![
            json!({"timestamp": "2024-01-15T10:00:00Z", "value": 1.0}),
            json!({"timestamp": "2024-01-15T10:01:00Z", "value": 2.0}),
            json!({"timestamp": "2024-01-15T10:02:00Z", "value": 3.0}),
        ];
        
        for entry in &entries {
            let entry_bytes = entry.to_string().into_bytes();
            wal.append(&entry_bytes).unwrap();
        }
        
        let replayed = wal.replay().unwrap();
        assert_eq!(replayed.len(), 3);
        
        for (i, replayed_entry) in replayed.iter().enumerate() {
            let replayed_json: serde_json::Value = 
                serde_json::from_slice(replayed_entry).unwrap();
            assert_eq!(replayed_json, entries[i]);
        }
        
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_commit_clears_log() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();
        
        for i in 0..3 {
            let entry = json!({"id": i});
            let entry_bytes = entry.to_string().into_bytes();
            wal.append(&entry_bytes).unwrap();
        }
        
        assert_eq!(wal.replay().unwrap().len(), 3);
        
        wal.commit().unwrap();
        
        let replayed_after_commit = wal.replay().unwrap();
        assert_eq!(replayed_after_commit.len(), 0);
        
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_append_after_commit() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();
        
        let entry1 = json!({"id": 1});
        wal.append(&entry1.to_string().into_bytes()).unwrap();
        
        wal.commit().unwrap();
        
        let entry2 = json!({"id": 2});
        wal.append(&entry2.to_string().into_bytes()).unwrap();
        
        let replayed = wal.replay().unwrap();
        assert_eq!(replayed.len(), 1);
        
        let replayed_json: serde_json::Value = 
            serde_json::from_slice(&replayed[0]).unwrap();
        assert_eq!(replayed_json, entry2);
        
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_empty_replay() {
        let path = temp_wal_path();
        let wal = WriteAheadLog::new(&path).unwrap();
        
        let replayed = wal.replay().unwrap();
        assert_eq!(replayed.len(), 0);
        
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_invalid_utf8() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();
        
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        let result = wal.append(&invalid_utf8);
        
        assert!(result.is_err());
        if let Err(CoreError::Storage(msg)) = result {
            assert!(msg.contains("Invalid UTF-8"));
        }
        
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_persistence_across_instances() {
        let path = temp_wal_path();
        
        {
            let mut wal1 = WriteAheadLog::new(&path).unwrap();
            let entry = json!({"data": "persistent"});
            wal1.append(&entry.to_string().into_bytes()).unwrap();
        }
        
        let wal2 = WriteAheadLog::new(&path).unwrap();
        let replayed = wal2.replay().unwrap();
        
        assert_eq!(replayed.len(), 1);
        let replayed_json: serde_json::Value = 
            serde_json::from_slice(&replayed[0]).unwrap();
        assert_eq!(replayed_json, json!({"data": "persistent"}));
        
        let _ = fs::remove_file(&path);
    }
}
