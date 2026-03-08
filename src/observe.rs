use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::eval::ExecutionMetrics;

pub const OBSERVABILITY_SUFFIX: &str = ".observability.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservabilityEntry {
    pub line: usize,
    pub count: u64,
    pub avg_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservabilityReport {
    pub file_hash: String,
    pub lines: Vec<ObservabilityEntry>,
}

pub struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut starts = vec![0];
        for (idx, ch) in source.char_indices() {
            if ch == '\n' {
                starts.push(idx + 1);
            }
        }
        Self { starts }
    }

    pub fn line_for_offset(&self, offset: usize) -> usize {
        self.starts.partition_point(|start| *start <= offset).max(1)
    }
}

pub fn hash_source(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn observability_path(source_path: &Path) -> PathBuf {
    let mut path = OsString::from(source_path.as_os_str());
    path.push(OBSERVABILITY_SUFFIX);
    PathBuf::from(path)
}

pub fn build_report(source: &str, metrics: &ExecutionMetrics) -> ObservabilityReport {
    let line_index = LineIndex::new(source);
    let mut by_line: BTreeMap<usize, (u64, u128)> = BTreeMap::new();

    for (offset, metric) in &metrics.by_offset {
        let line = line_index.line_for_offset(*offset);
        let entry = by_line.entry(line).or_insert((0, 0));
        entry.0 += metric.count;
        entry.1 += metric.total_nanos;
    }

    let lines = by_line
        .into_iter()
        .map(|(line, (count, total_nanos))| ObservabilityEntry {
            line,
            count,
            avg_nanos: if count == 0 {
                0
            } else {
                (total_nanos / u128::from(count)).min(u128::from(u64::MAX)) as u64
            },
        })
        .collect();

    ObservabilityReport {
        file_hash: hash_source(source),
        lines,
    }
}

pub fn write_report(source_path: &Path, report: &ObservabilityReport) -> io::Result<()> {
    let path = observability_path(source_path);
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{ExecutionMetric, ExecutionMetrics};

    #[test]
    fn line_index_maps_offsets_to_lines() {
        let index = LineIndex::new("alpha\nbeta\ngamma");

        assert_eq!(index.line_for_offset(0), 1);
        assert_eq!(index.line_for_offset(6), 2);
        assert_eq!(index.line_for_offset(11), 3);
    }

    #[test]
    fn build_report_aggregates_offsets_by_line() {
        let mut metrics = ExecutionMetrics::default();
        metrics.by_offset.insert(
            0,
            ExecutionMetric {
                count: 2,
                total_nanos: 20,
            },
        );
        metrics.by_offset.insert(
            2,
            ExecutionMetric {
                count: 1,
                total_nanos: 30,
            },
        );
        metrics.by_offset.insert(
            5,
            ExecutionMetric {
                count: 3,
                total_nanos: 60,
            },
        );

        let report = build_report("abcd\nef", &metrics);

        assert_eq!(
            report.lines,
            vec![
                ObservabilityEntry {
                    line: 1,
                    count: 3,
                    avg_nanos: 16,
                },
                ObservabilityEntry {
                    line: 2,
                    count: 3,
                    avg_nanos: 20,
                },
            ]
        );
    }
}
