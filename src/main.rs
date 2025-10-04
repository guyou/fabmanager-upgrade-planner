use regex::Regex;
use reqwest::Error;
use reqwest::blocking::get;
use std::collections::HashMap;

#[derive(Debug)]
struct ChangelogEntry {
    date: String,
    changes: Vec<String>,
}

fn fetch_changelog(url: &str) -> Result<String, Error> {
    let response = get(url)?.text()?;
    Ok(response)
}

fn parse_changelog(content: &str) -> HashMap<String, ChangelogEntry> {
    // Regex to match version entries (assuming format '## [version] - YYYY-MM-DD')
    let re = Regex::new(r"## (v.*?) (.*)").unwrap();

    let mut entries = HashMap::new();
    let mut current_version = String::new();
    let mut current_date = String::new();
    let mut current_changes = Vec::new();

    for line in content.lines() {
        if line.starts_with("## ") {
            // Store the previous entry before moving to the next
            if !current_version.is_empty() {
                entries.insert(
                    current_version.clone(),
                    ChangelogEntry {
                        date: current_date.clone(),
                        changes: current_changes,
                    },
                );
                current_changes = Vec::new(); // Clear the previous content for the next entry
            }
            if let Some(cap) = re.captures(line) {
                current_version = cap[1].to_string();
                current_date = cap[2].to_string();
            }
        } else if line.starts_with("- ") {
            // Append the content for the current version
            current_changes.push(line.to_string());
        }
    }

    // Store the last entry after loop ends
    if !current_version.is_empty() {
        entries.insert(
            current_version,
            ChangelogEntry {
                date: current_date,
                changes: current_changes,
            },
        );
    }

    entries
}

fn main() {
    let url = "https://raw.githubusercontent.com/sleede/fab-manager/refs/heads/master/CHANGELOG.md"; // Replace with your URL

    match fetch_changelog(url) {
        Ok(content) => {
            let changelog_entries = parse_changelog(&content);
            for (version, entry) in changelog_entries {
                println!(
                    "Version: {}\nDate: {}\nContent:\n{:?}\n",
                    version, entry.date, entry.changes
                );
            }
        }
        Err(e) => eprintln!("Error fetching changelog: {}", e),
    }
}
