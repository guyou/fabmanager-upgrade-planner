# fabmanager-upgrade-planner

## Overview
**fabmanager-upgrade-planner** is a Rust application designed to calculate an upgrade plan for [Fab Manager](https://www.fab-manager.com/fr). It helps users effectively manage and plan upgrades to their Fab Manager environments, ensuring optimal performance and the latest features.

## Features
- reads the Changelog to identify the version requiring some manual adjustments
- fetch the upgrade command from release notes

## Requirements
- Rust (1.48 or higher)
- Cargo

## Installation

### Clone the Repository
```bash
git clone https://github.com/guyou/fabmanager-upgrade-planner.git
cd fabmanager-upgrade-planner
```

### Build the Application
```bash
cargo build --release
```

## Usage

### Running the Application
To start the application, run:
```bash
cargo run --release
```

### Command-Line Arguments


#### Example
```bash
cargo run --release -- --from "4.7.14"
```

This command first fetch for the next release (here `5.6.11`) and then generates an upgrade plan from version `4.7.14` to `5.6.11`.

```bash
cargo run --release -- --from "4.7.14" --to "5.6.11"
```

This command generates an upgrade plan from version `4.7.14` to `5.6.11`.

## Contributing
Contributions are welcome! If you have suggestions for improvements, please fork the repository and submit a pull request. Before contributing, please ensure you follow these guidelines:

1. Ensure your code compiles without errors.
2. Write clear, descriptive commit messages.
3. Update the documentation for any new features or changes.

## License
This project is licensed under the GPL License - see the [LICENSE](LICENSE) file for details.
