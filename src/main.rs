//use reqwest::blocking::Client;
use clap::Parser;
use log::{debug, error};
use reqwest::Client;
use semver::Version;

mod fabmanager;

#[derive(Parser)]
struct Cli {
    /// The initial version
    #[arg(short, long)]
    from: String,
    /// The target version
    #[arg(short, long)]
    to: Option<String>,
    /// Fetch release
    #[arg(short, long, default_value_t = false)]
    disable_release: bool,
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
        match fabmanager::fetch_next(&client, &args.from).await {
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

    match fabmanager::fetch_changelog(&client).await {
        Ok(content) => {
            let mut options: Vec<String> = Vec::new();
            let mut todos: Vec<String> = Vec::new();
            let changelog_entries = fabmanager::parse_changelog(&content);
            for entry in changelog_entries {
                let raw_version = entry.version.strip_prefix("v").unwrap();
                let v = Version::parse(raw_version).unwrap();
                debug!("Found version {}", v);
                if from_version.lt(&v) && to_version.ge(&v) {
                    debug!(
                        "Version: {}\nDate: {}\nContent:\n{:?}\n",
                        entry.version, entry.date, entry.changes
                    );
                    let deploy_entries: Vec<String> = fabmanager::extract_todos(&entry);
                    if deploy_entries.is_empty() {
                        debug!("No todo");
                        continue;
                    }
                    for todo in deploy_entries {
                        if !todos.contains(&todo.to_string()) {
                            todos.push(todo.to_string());
                        }
                    }
                    if !args.disable_release {
                        let release = fabmanager::fetch_release(&client, &entry.version).await.unwrap();
                        if let Some(release) = fabmanager::parse_release(&release) {
                            let upgrade_cmd = release
                                .update
                                .replace(" -- ", format!(" -- -t {} ", raw_version).as_str());
                            println!("Update to release {}:\n{}", entry.version, upgrade_cmd);
                            let current_opts = fabmanager::extract_options(release.update.as_str());
                            debug!("Found options {:?}", current_opts);
                            for opt in current_opts {
                                if !options.contains(&opt.to_string()) {
                                    debug!("Added {}",opt.to_string());
                                    options.push(opt.to_string());
                                } else {
                                    debug!("Ignored {}",opt.to_string());
                                }
                            }
                        } else {
                            error!("No update found for {}", entry.version);
                        }
                    }
                }
            }
            println!(
                "Todos:\n{}",
                todos
                    .iter()
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            println!(
                "Command:\n\\curl -sSL upgrade.fab.mn | bash -s -- -t {} {}",
                to_version,
                options.join(" ")
            );
        }
        Err(e) => eprintln!("Error fetching changelog: {}", e),
    }
}
