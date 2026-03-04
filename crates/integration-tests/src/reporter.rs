//! Simple test result reporter that writes JSON and markdown reports.

use serde::Serialize;
use std::io::Write;

/// Outcome of a single test execution.
#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub test_name: String,
    pub suite: String,
    pub duration_ms: u64,
    pub passed: bool,
    pub error_message: Option<String>,
}

/// Aggregated report for one iteration of the test suite.
#[derive(Debug, Clone, Serialize)]
pub struct IterationReport {
    pub iteration: u32,
    pub timestamp: String,
    pub results: Vec<TestResult>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rate: f64,
}

impl IterationReport {
    /// Build a report from a list of test results. Counts and pass rate are
    /// computed automatically.
    pub fn new(iteration: u32, results: Vec<TestResult>) -> Self {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        let pass_rate = if total > 0 {
            passed as f64 / total as f64
        } else {
            0.0
        };
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self {
            iteration,
            timestamp,
            results,
            total,
            passed,
            failed,
            pass_rate,
        }
    }

    /// Write the report as a JSON file inside `dir`.
    ///
    /// File name: `report_iter_{iteration}.json`
    pub fn write_json(&self, dir: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = format!("{}/report_iter_{}.json", dir, self.iteration);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut f = std::fs::File::create(&path)?;
        f.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Write the report as a Markdown file inside `dir`.
    ///
    /// File name: `report_iter_{iteration}.md`
    pub fn write_markdown(&self, dir: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = format!("{}/report_iter_{}.md", dir, self.iteration);
        let mut f = std::fs::File::create(&path)?;

        writeln!(f, "# Test Report - Iteration {}", self.iteration)?;
        writeln!(f)?;
        writeln!(f, "**Timestamp:** {}", self.timestamp)?;
        writeln!(
            f,
            "**Results:** {} passed, {} failed, {} total ({:.1}%)",
            self.passed,
            self.failed,
            self.total,
            self.pass_rate * 100.0
        )?;
        writeln!(f)?;
        writeln!(f, "| Suite | Test | Duration (ms) | Result | Error |")?;
        writeln!(f, "|-------|------|--------------|--------|-------|")?;

        for r in &self.results {
            let status = if r.passed { "PASS" } else { "FAIL" };
            let err = r
                .error_message
                .as_deref()
                .unwrap_or("-");
            writeln!(
                f,
                "| {} | {} | {} | {} | {} |",
                r.suite, r.test_name, r.duration_ms, status, err
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_counts_are_correct() {
        let results = vec![
            TestResult {
                test_name: "test_a".into(),
                suite: "suite1".into(),
                duration_ms: 100,
                passed: true,
                error_message: None,
            },
            TestResult {
                test_name: "test_b".into(),
                suite: "suite1".into(),
                duration_ms: 200,
                passed: false,
                error_message: Some("assertion failed".into()),
            },
        ];
        let report = IterationReport::new(1, results);
        assert_eq!(report.total, 2);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 1);
        assert!((report.pass_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn write_json_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let report = IterationReport::new(0, vec![]);
        report
            .write_json(dir.path().to_str().unwrap())
            .expect("write_json failed");
        let path = dir.path().join("report_iter_0.json");
        assert!(path.exists());
    }

    #[test]
    fn write_markdown_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let report = IterationReport::new(0, vec![]);
        report
            .write_markdown(dir.path().to_str().unwrap())
            .expect("write_markdown failed");
        let path = dir.path().join("report_iter_0.md");
        assert!(path.exists());
    }
}
