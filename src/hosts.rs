use anyhow::{Context, Result};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A host with associated tags
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedHost {
    pub hostname: String,
    pub tags: HashSet<String>,
}

/// Tag filter: OR of AND-groups
#[derive(Debug, Clone, PartialEq)]
pub enum TagFilter {
    /// Match all hosts
    All,
    /// OR of AND-groups: [[a, b], [c]] means (a AND b) OR c
    Groups(Vec<Vec<String>>),
}

/// Resolve hosts from source with optional tag filter
///
/// Source formats:
/// - `None`: Load from default hosts file (~/.config/bdsh/hosts)
/// - `@"cmd arg1 arg2"`: Run shell command, parse output
/// - `@/path/to/file`: Read file (or execute if has x-bit)
/// - `host1,host2,host3`: Inline comma-separated hostnames
///
/// Filter formats:
/// - `None`: All hosts
/// - `:tag`: Hosts with tag
/// - `:t1:t2`: Hosts with t1 AND t2
/// - `:t1,:t2`: Hosts with t1 OR t2
/// - `:t1:t2,:t3`: Hosts with (t1 AND t2) OR t3
pub fn resolve_hosts(source: Option<&str>, filter: Option<&str>) -> Result<Vec<String>> {
    // Validate filter first - `:` alone is an error
    if let Some(f) = filter {
        if f == ":" {
            anyhow::bail!("Empty tag filter: use tags like :web or :prod, not ':'");
        }
    }

    // Load hosts from source
    let hosts = match source {
        None => load_config()?,
        Some(s) if s.starts_with('@') => {
            let path_or_cmd = &s[1..];
            let path = Path::new(path_or_cmd);

            let content = if path.exists() {
                // If path exists, read it or execute it (if executable)
                if is_executable(path)? {
                    execute_script(path)?
                } else {
                    fs::read_to_string(path)
                        .with_context(|| format!("Failed to read: {}", path.display()))?
                }
            } else {
                // If path doesn't exist, treat as shell command
                run_shell_command(path_or_cmd)?
            };
            parse_tagged_lines(&content)
        }
        Some(s) => {
            // Inline comma-separated - no tags, just hostnames
            return Ok(parse_inline(s));
        }
    };

    // Apply filter
    let filtered = match filter {
        None => hosts,
        Some(f) => {
            let tag_filter = parse_tag_filter(f)?;
            hosts
                .into_iter()
                .filter(|h| matches_filter(h, &tag_filter))
                .collect()
        }
    };

    if filtered.is_empty() {
        anyhow::bail!("No hosts match filter");
    }

    Ok(filtered.into_iter().map(|h| h.hostname).collect())
}

/// Run a shell command and return its stdout
fn run_shell_command(cmd: &str) -> Result<String> {
    let output = Command::new("sh")
        .args(["-c", cmd])
        .output()
        .with_context(|| format!("Failed to run command: {}", cmd))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Command failed: {}", stderr.trim());
    }

    String::from_utf8(output.stdout).context("Invalid UTF-8 output from command")
}

/// Get the default hosts file path
fn config_path() -> Option<PathBuf> {
    let config_dir = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });

    let path = config_dir.join("bdsh").join("hosts");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Load hosts from config file
fn load_config() -> Result<Vec<TaggedHost>> {
    let path = config_path().context("No hosts file found at ~/.config/bdsh/hosts")?;

    let contents = if is_executable(&path)? {
        execute_script(&path)?
    } else {
        fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?
    };

    Ok(parse_tagged_lines(&contents))
}

/// Check if a file is executable
fn is_executable(path: &Path) -> Result<bool> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to access: {}", path.display()))?;

    Ok(metadata.is_file() && (metadata.permissions().mode() & 0o111) != 0)
}

/// Execute a script and return its stdout
fn execute_script(path: &Path) -> Result<String> {
    let output = Command::new(path)
        .output()
        .with_context(|| format!("Failed to execute: {}", path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Script {} failed with {}: {}",
            path.display(),
            output.status,
            stderr.trim()
        );
    }

    String::from_utf8(output.stdout)
        .with_context(|| format!("Invalid UTF-8 output from: {}", path.display()))
}

/// Parse inline comma-separated hosts (no tags supported)
fn parse_inline(spec: &str) -> Vec<String> {
    spec.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse tagged lines format: `hostname [:tag1] [:tag2] ...`
/// Lines starting with `#`, `//`, or `;` are treated as comments and ignored.
fn parse_tagged_lines(content: &str) -> Vec<TaggedHost> {
    content
        .lines()
        .map(|line| line.trim())
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("//")
                && !line.starts_with(';')
        })
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let hostname = parts[0].to_string();
            let tags: HashSet<String> = parts[1..]
                .iter()
                .filter(|p| p.starts_with(':'))
                .map(|p| p[1..].to_string())
                .collect();
            TaggedHost { hostname, tags }
        })
        .collect()
}

/// Parse a tag filter specification
///
/// Format: `:tag`, `:t1:t2` (AND), `:t1,:t2` (OR), or combinations
/// - `:a:b` means must have a AND b
/// - `:a,:b:c` means must have a OR (b AND c)
fn parse_tag_filter(spec: &str) -> Result<TagFilter> {
    let spec = spec.strip_prefix(':').unwrap_or(spec);
    if spec.is_empty() {
        return Ok(TagFilter::All);
    }

    let groups: Vec<Vec<String>> = spec
        .split(',')
        .map(|group| {
            group
                .split(':')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .filter(|g: &Vec<String>| !g.is_empty())
        .collect();

    if groups.is_empty() {
        Ok(TagFilter::All)
    } else {
        Ok(TagFilter::Groups(groups))
    }
}

/// Check if a host matches a tag filter
fn matches_filter(host: &TaggedHost, filter: &TagFilter) -> bool {
    match filter {
        TagFilter::All => true,
        TagFilter::Groups(groups) => {
            // OR across groups, AND within each group
            groups
                .iter()
                .any(|group| group.iter().all(|tag| host.tags.contains(tag)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // === Inline parsing tests ===

    #[test]
    fn parse_inline_single() {
        let hosts = parse_inline("host1");
        assert_eq!(hosts, vec!["host1"]);
    }

    #[test]
    fn parse_inline_multiple() {
        let hosts = parse_inline("host1,host2,host3");
        assert_eq!(hosts, vec!["host1", "host2", "host3"]);
    }

    #[test]
    fn parse_inline_with_spaces() {
        let hosts = parse_inline("host1, host2 , host3");
        assert_eq!(hosts, vec!["host1", "host2", "host3"]);
    }

    // === Tagged lines parsing tests ===

    #[test]
    fn parse_tagged_lines_simple() {
        let hosts = parse_tagged_lines("host1 :web\nhost2 :db\n");
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].hostname, "host1");
        assert!(hosts[0].tags.contains("web"));
        assert_eq!(hosts[1].hostname, "host2");
        assert!(hosts[1].tags.contains("db"));
    }

    #[test]
    fn parse_tagged_lines_multiple_tags() {
        let hosts = parse_tagged_lines("host1 :web :prod\n");
        assert_eq!(hosts.len(), 1);
        assert!(hosts[0].tags.contains("web"));
        assert!(hosts[0].tags.contains("prod"));
    }

    #[test]
    fn parse_tagged_lines_no_tags() {
        let hosts = parse_tagged_lines("host1\nhost2\n");
        assert_eq!(hosts.len(), 2);
        assert!(hosts[0].tags.is_empty());
    }

    #[test]
    fn parse_tagged_lines_with_comments() {
        let hosts = parse_tagged_lines("# comment\nhost1 :web\n\n# another\nhost2\n");
        assert_eq!(hosts.len(), 2);
    }

    #[test]
    fn parse_tagged_lines_with_various_comment_styles() {
        let hosts = parse_tagged_lines(
            "# hash comment\nhost1 :web\n// slash comment\nhost2 :db\n; semicolon comment\nhost3 :api\n",
        );
        assert_eq!(hosts.len(), 3);
        assert_eq!(hosts[0].hostname, "host1");
        assert_eq!(hosts[1].hostname, "host2");
        assert_eq!(hosts[2].hostname, "host3");
    }

    #[test]
    fn parse_tagged_lines_with_whitespace_only_lines() {
        let hosts = parse_tagged_lines(
            "host1 :web\n   \n\t\t\nhost2 :db\n  \t  \nhost3 :api\n",
        );
        assert_eq!(hosts.len(), 3);
        assert_eq!(hosts[0].hostname, "host1");
        assert_eq!(hosts[1].hostname, "host2");
        assert_eq!(hosts[2].hostname, "host3");
    }

    // === Tag filter parsing tests ===

    #[test]
    fn parse_tag_filter_single() {
        let filter = parse_tag_filter(":web").unwrap();
        assert_eq!(filter, TagFilter::Groups(vec![vec!["web".to_string()]]));
    }

    #[test]
    fn parse_tag_filter_and() {
        let filter = parse_tag_filter(":web:prod").unwrap();
        assert_eq!(
            filter,
            TagFilter::Groups(vec![vec!["web".to_string(), "prod".to_string()]])
        );
    }

    #[test]
    fn parse_tag_filter_or() {
        let filter = parse_tag_filter(":web,:db").unwrap();
        assert_eq!(
            filter,
            TagFilter::Groups(vec![vec!["web".to_string()], vec!["db".to_string()]])
        );
    }

    #[test]
    fn parse_tag_filter_complex() {
        let filter = parse_tag_filter(":web:prod,:db").unwrap();
        assert_eq!(
            filter,
            TagFilter::Groups(vec![
                vec!["web".to_string(), "prod".to_string()],
                vec!["db".to_string()]
            ])
        );
    }

    #[test]
    fn parse_tag_filter_empty_becomes_all() {
        let filter = parse_tag_filter("").unwrap();
        assert_eq!(filter, TagFilter::All);
    }

    // === Filter matching tests ===

    #[test]
    fn matches_filter_all() {
        let host = TaggedHost {
            hostname: "h1".to_string(),
            tags: HashSet::new(),
        };
        assert!(matches_filter(&host, &TagFilter::All));
    }

    #[test]
    fn matches_filter_single_tag() {
        let host = TaggedHost {
            hostname: "h1".to_string(),
            tags: HashSet::from(["web".to_string()]),
        };
        let filter = TagFilter::Groups(vec![vec!["web".to_string()]]);
        assert!(matches_filter(&host, &filter));
    }

    #[test]
    fn matches_filter_and_logic() {
        let host = TaggedHost {
            hostname: "h1".to_string(),
            tags: HashSet::from(["web".to_string(), "prod".to_string()]),
        };
        let filter = TagFilter::Groups(vec![vec!["web".to_string(), "prod".to_string()]]);
        assert!(matches_filter(&host, &filter));

        // Missing one tag
        let host2 = TaggedHost {
            hostname: "h2".to_string(),
            tags: HashSet::from(["web".to_string()]),
        };
        assert!(!matches_filter(&host2, &filter));
    }

    #[test]
    fn matches_filter_or_logic() {
        let host = TaggedHost {
            hostname: "h1".to_string(),
            tags: HashSet::from(["db".to_string()]),
        };
        let filter = TagFilter::Groups(vec![vec!["web".to_string()], vec!["db".to_string()]]);
        assert!(matches_filter(&host, &filter));
    }

    // === File-based tests ===

    #[test]
    fn resolve_hosts_from_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "host1 :web").unwrap();
        writeln!(file, "host2 :db").unwrap();
        writeln!(file, "host3 :web :db").unwrap();

        let spec = format!("@{}", file.path().display());
        let hosts = resolve_hosts(Some(&spec), None).unwrap();
        assert_eq!(hosts, vec!["host1", "host2", "host3"]);
    }

    #[test]
    fn resolve_hosts_from_file_with_filter() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "host1 :web").unwrap();
        writeln!(file, "host2 :db").unwrap();
        writeln!(file, "host3 :web :db").unwrap();

        let spec = format!("@{}", file.path().display());
        let hosts = resolve_hosts(Some(&spec), Some(":web")).unwrap();
        assert_eq!(hosts, vec!["host1", "host3"]);
    }

    #[test]
    fn resolve_hosts_from_file_with_and_filter() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "host1 :web").unwrap();
        writeln!(file, "host2 :db").unwrap();
        writeln!(file, "host3 :web :db").unwrap();

        let spec = format!("@{}", file.path().display());
        let hosts = resolve_hosts(Some(&spec), Some(":web:db")).unwrap();
        assert_eq!(hosts, vec!["host3"]);
    }

    #[test]
    fn resolve_hosts_from_file_with_or_filter() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "host1 :web").unwrap();
        writeln!(file, "host2 :db").unwrap();
        writeln!(file, "host3 :cache").unwrap();

        let spec = format!("@{}", file.path().display());
        let hosts = resolve_hosts(Some(&spec), Some(":web,:db")).unwrap();
        assert_eq!(hosts, vec!["host1", "host2"]);
    }

    #[test]
    fn resolve_hosts_inline() {
        let hosts = resolve_hosts(Some("h1,h2,h3"), None).unwrap();
        assert_eq!(hosts, vec!["h1", "h2", "h3"]);
    }

    #[test]
    fn resolve_hosts_empty_tag_error() {
        let result = resolve_hosts(None, Some(":"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty tag"));
    }

    #[test]
    fn resolve_hosts_from_executable() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "#!/bin/sh").unwrap();
        writeln!(file, "echo 'host1 :a'").unwrap();
        writeln!(file, "echo 'host2 :b'").unwrap();

        // Close the file handle to avoid "Text file busy" error when executing
        let path = file.into_temp_path();

        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();

        let spec = format!("@{}", path.display());
        let hosts = resolve_hosts(Some(&spec), None).unwrap();
        assert_eq!(hosts, vec!["host1", "host2"]);
    }

    #[test]
    fn resolve_hosts_from_shell_command() {
        // Simulates: bdsh @'echo h1; echo h2' (shell strips quotes, bdsh gets command)
        let hosts = resolve_hosts(Some("@echo 'h1 :a'; echo 'h2 :b'"), None).unwrap();
        assert_eq!(hosts, vec!["h1", "h2"]);
    }

    #[test]
    fn resolve_hosts_from_shell_command_with_filter() {
        // Simulates: bdsh @'echo h1; echo h2' :a (shell strips quotes)
        let hosts = resolve_hosts(Some("@echo 'h1 :a'; echo 'h2 :b'"), Some(":a")).unwrap();
        assert_eq!(hosts, vec!["h1"]);
    }

    #[test]
    fn resolve_hosts_no_match_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "host1 :web").unwrap();

        let spec = format!("@{}", file.path().display());
        let result = resolve_hosts(Some(&spec), Some(":nonexistent"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No hosts match"));
    }

    #[test]
    fn resolve_hosts_file_not_found() {
        let result = resolve_hosts(Some("@/nonexistent/file.txt"), None);
        assert!(result.is_err());
    }
}
