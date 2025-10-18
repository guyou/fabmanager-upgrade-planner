use regex::Regex;
use reqwest::Error;
//use reqwest::blocking::Client;
use clap::Parser;
use log::{debug, error, info};
use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use std::collections::VecDeque;

#[derive(Deserialize, Debug)]
struct NextRelease {
    semver: String,
    url: String,
}

#[derive(Deserialize, Debug)]
struct NextResponse {
    next_step: NextRelease,
}

async fn fetch_next(client: &Client, current: &str) -> Result<NextRelease, Error> {
    let url = format!(
        "https://hub.fab-manager.com/api/versions/next_step?version={}",
        current
    );
    let response = client.get(url).send().await.unwrap();
    let response = response.json::<NextResponse>().await.unwrap();
    Ok(response.next_step)
}

#[derive(Debug)]
struct ChangelogEntry {
    version: String,
    date: String,
    changes: Vec<String>,
}

async fn fetch_changelog(client: &Client) -> Result<String, Error> {
    let url = "https://raw.githubusercontent.com/sleede/fab-manager/refs/heads/master/CHANGELOG.md";
    let response = client.get(url).send().await.unwrap().text().await.unwrap();
    Ok(response)
}

fn parse_changelog(content: &str) -> VecDeque<ChangelogEntry> {
    // Regex to match version entries (assuming format '## [version] - YYYY-MM-DD')
    let re = Regex::new(r"## (v.*?) (.*)").unwrap();

    let mut entries = VecDeque::new();
    let mut current_version = String::new();
    let mut current_date = String::new();
    let mut current_changes = Vec::new();

    for line in content.lines() {
        if line.starts_with("## ") {
            // Store the previous entry before moving to the next
            if !current_version.is_empty() {
                entries.push_front(ChangelogEntry {
                    version: current_version.clone(),
                    date: current_date.clone(),
                    changes: current_changes,
                });
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
        entries.push_front(ChangelogEntry {
            version: current_version.clone(),
            date: current_date,
            changes: current_changes,
        });
    }

    entries
}

#[derive(Deserialize, Debug)]
struct ReleaseResponse {
    body: String,
}

#[derive(Debug)]
struct Release {
    update: String,
}

async fn fetch_release(client: &Client, tag: &str) -> Result<String, Error> {
    let url = format!(
        "https://api.github.com/repos/sleede/fab-manager/releases/tags/{}",
        tag
    );
    debug!("Fetch '{}'", url);
    let response = client
        .get(url)
        .header("User-Agent", "curl/8.5.0")
        .header("Accept", "application/json")
        .send()
        .await
        .unwrap();

    debug!("Response status: {}", response.status());
    debug!("Response headers: {:?}", response.headers());

    let response = response.json::<ReleaseResponse>().await.unwrap();
    Ok(response.body)
}

fn parse_release(content: &str) -> Option<Release> {
    let re = Regex::new(r".*## \[UPDATE\].*\s*```bash\s*(.*?)\s*```").unwrap();

    if let Some(cap) = re.captures(content) {
        Some(Release {
            update: cap[1].to_string(),
        })
    } else {
        None
    }
}

#[derive(Parser)]
struct Cli {
    /// The initial version
    #[arg(short, long)]
    from: String,
    /// The target version
    #[arg(short, long)]
    to: Option<String>,
}

fn extract_options(cmd: &str) -> Vec<&str> {
    let mut res = Vec::new();
    if let Some(args) = cmd.split(" -- ").nth(1) {
        // Create a regex that matches the delimiters
        let re = Regex::new(r#"\s*(-s|-e|-c|-p)\s*\"[^\"]*\""#).unwrap();
        res = re.find_iter(args).map(|m| m.as_str().trim()).collect();
    } else {
        error!("no arguments found in {}", cmd);
    }
    res
}

#[tokio::main]
async fn main() {
    env_logger::init();
    // Collect the arguments into a vector
    let args = Cli::parse();

    let client = Client::new();

    let from_version = Version::parse(&args.from);
    if let Err(e) = from_version {
        eprintln!("Error parsing version {}: {}", args.from, e);
        return;
    }
    let from_version = from_version.unwrap();

    let to: String;
    if let Some(target_version) = args.to {
        to = target_version;
    } else {
        match fetch_next(&client, &args.from).await {
            Ok(target_version) => {
                to = target_version.semver;
            }
            Err(e) => {
                eprintln!("Error fetching next release: {}", e);
                return;
            }
        }
    }
    let to_version = Version::parse(&to);
    if let Err(e) = to_version {
        eprintln!("Error parsing version {}: {}", to, e);
        return;
    }
    let to_version = to_version.unwrap();

    match fetch_changelog(&client).await {
        Ok(content) => {
            let mut options: Vec<String> = Vec::new();
            let changelog_entries = parse_changelog(&content);
            for entry in changelog_entries {
                let raw_version = entry.version.strip_prefix("v").unwrap();
                let v = Version::parse(raw_version).unwrap();
                debug!("Found version {}", v);
                if from_version.lt(&v) && to_version.ge(&v) {
                    debug!(
                        "Version: {}\nDate: {}\nContent:\n{:?}\n",
                        entry.version, entry.date, entry.changes
                    );
                    let contains_todo = entry.changes.iter().any(|s| s.contains("[TODO DEPLOY]"));
                    if !contains_todo {
                        debug!("No todo");
                        continue;
                    }
                    info!(
                        "Version: {}\nDate: {}\nContent:\n{:?}\n",
                        entry.version, entry.date, entry.changes
                    );
                    let release = fetch_release(&client, &entry.version).await.unwrap();
                    if let Some(release) = parse_release(&release) {
                        let upgrade_cmd = release
                            .update
                            .replace(" -- ", format!(" -- -t {} ", raw_version).as_str());
                        println!("Update to release {}:\n{}", entry.version, upgrade_cmd);
                        let current_opts = extract_options(release.update.as_str());
                        for opt in current_opts {
                            if !options.contains(&opt.to_string()) {
                                options.push(opt.to_string());
                            }
                        }
                    } else {
                        error!("No update found for {}", entry.version);
                    }
                }
            }
            println!(
                "Command:\n\\curl -sSL upgrade.fab.mn | bash -s -- -t {} {}",
                to_version,
                options.join(" ")
            );
        }
        Err(e) => eprintln!("Error fetching changelog: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_options() {
        let options = extract_options(
            "bash -- -c \"rails fablab:stripe:set_gateway\" -c \"rails fablab:maintenance:rebuild_stylesheet\" -s \"rename-adminsys\"",
        );
        print!("{:?}", options);
        assert_eq!(3, options.len());
    }

    #[test]
    fn it_works() {
        let json_response = r#"
        {
        "url": "https://api.github.com/repos/sleede/fab-manager/releases/44420026",
        "assets_url": "https://api.github.com/repos/sleede/fab-manager/releases/44420026/assets",
        "upload_url": "https://uploads.github.com/repos/sleede/fab-manager/releases/44420026/assets{?name,label}",
        "html_url": "https://github.com/sleede/fab-manager/releases/tag/v5.0.0",
        "id": 44420026,
        "author": {
            "login": "sylvainbx",
            "id": 5102799,
            "node_id": "MDQ6VXNlcjUxMDI3OTk=",
            "avatar_url": "https://avatars.githubusercontent.com/u/5102799?v=4",
            "gravatar_id": "",
            "url": "https://api.github.com/users/sylvainbx",
            "html_url": "https://github.com/sylvainbx",
            "followers_url": "https://api.github.com/users/sylvainbx/followers",
            "following_url": "https://api.github.com/users/sylvainbx/following{/other_user}",
            "gists_url": "https://api.github.com/users/sylvainbx/gists{/gist_id}",
            "starred_url": "https://api.github.com/users/sylvainbx/starred{/owner}{/repo}",
            "subscriptions_url": "https://api.github.com/users/sylvainbx/subscriptions",
            "organizations_url": "https://api.github.com/users/sylvainbx/orgs",
            "repos_url": "https://api.github.com/users/sylvainbx/repos",
            "events_url": "https://api.github.com/users/sylvainbx/events{/privacy}",
            "received_events_url": "https://api.github.com/users/sylvainbx/received_events",
            "type": "User",
            "user_view_type": "public",
            "site_admin": false
        },
        "node_id": "MDc6UmVsZWFzZTQ0NDIwMDI2",
        "tag_name": "v5.0.0",
        "target_commitish": "master",
        "name": "Release 5.0.0",
        "draft": false,
        "immutable": false,
        "prerelease": false,
        "created_at": "2021-06-10T14:21:33Z",
        "updated_at": "2021-06-16T08:52:05Z",
        "published_at": "2021-06-10T14:41:52Z",
        "assets": [

        ],
        "tarball_url": "https://api.github.com/repos/sleede/fab-manager/tarball/v5.0.0",
        "zipball_url": "https://api.github.com/repos/sleede/fab-manager/zipball/v5.0.0",
        "body": "- [Ability to use PayZen to process online payments as an alternative to Stripe](https://feedback.fab-manager.com/posts/4/use-an-alternative-payment-gateway)\r\n- Ability to organize plans in categories\r\n- [Filter plans by group and by duration](https://feedback.fab-manager.com/posts/88/filter-plans-by-duration)\r\n- For payment schedules, ability to update the related payment card before the deadline\r\n- Improved the upgrade script\r\n- Refactored data architecture to make it generic\r\n- Various bug fixes, minor improvements and security fixes\r\n\r\nPlease read [the change log](CHANGELOG.md) for more details.\r\n\r\n#### BREAKING CHANGE\r\n`GET open_api/v1/invoices` won't return the exact same data structure anymore:\r\n- `stp_invoice_id` or `stp_payment_intent_id` has been replaced by `payment_gateway_object.id`.\r\n- `invoiced_id`, `invoiced_type` and `invoiced.created_at` has been replaced by `main_object:{type, id, created_at}`.\r\n\r\n#### [UPDATE](https://github.com/sleede/fab-manager/blob/master/doc/production_readme.md#update-fabmanager) 🪄\r\n```bash\r\n\\curl -sSL upgrade.fab.mn | bash -s -- -p \"rails fablab:chain:all\" -c \"rails fablab:stripe:set_gateway\" -c \"rails fablab:maintenance:rebuild_stylesheet\" -s \"rename-adminsys\"\r\n```"
        }"#;
        let release: ReleaseResponse = serde_json::from_str(json_response).unwrap();
        //assert_eq!(, 4);
        println!("{:}", release.body);
        let release = parse_release(&release.body).unwrap();
        assert_eq!(
            release.update,
            "\\curl -sSL upgrade.fab.mn | bash -s -- -p \"rails fablab:chain:all\" -c \"rails fablab:stripe:set_gateway\" -c \"rails fablab:maintenance:rebuild_stylesheet\" -s \"rename-adminsys\""
        );
    }

    #[test]
    fn it_works_2() {
        let release = parse_release("## [UPDATE]() \n```bash\nupdate\n``` ").unwrap();
        assert_eq!(release.update, "update");
    }

    #[test]
    fn it_works_2a() {
        let release = parse_release("## [UPDATE]() \n```bash\nupdate\n``` ").unwrap();
        assert_eq!(release.update, "update");
    }

    #[test]
    fn it_works_3() {
        let body = r#"
 #### [UPDATE](https://github.com/sleede/fab-manager/blob/master/doc/production_readme.md#update-fabmanager) 🪄
```bash
\curl -sSL upgrade.fab.mn | bash -s -- -p "rails fablab:chain:all" -c "rails fablab:stripe:set_gateway" -c "rails fablab:maintenance:rebuild_stylesheet" -s "rename-adminsys"
```
       "#;
        let release = parse_release(body).unwrap();
        assert_eq!(
            release.update,
            "\\curl -sSL upgrade.fab.mn | bash -s -- -p \"rails fablab:chain:all\" -c \"rails fablab:stripe:set_gateway\" -c \"rails fablab:maintenance:rebuild_stylesheet\" -s \"rename-adminsys\""
        );
    }
}
