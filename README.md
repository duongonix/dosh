# dosh

**The Developer's Optimized SHell.** *A fast, safe, and cross-platform command-line environment engineered in Rust.*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-v1.75+-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](#installation)

---

## 🐚 Introduction

**dosh** is a high-performance shell designed for the modern developer. Built from the ground up using **Rust**, it prioritizes memory safety, blazing speed, and cross-platform consistency. 

In an era where developers often jump between Windows, macOS, and Linux, **dosh** provides a unified interface. It brings the power of Unix-like command structures to every environment, integrates seamlessly with native PowerShell utilities, and offers a visually stunning, informative prompt right out of the box. Whether you are automating workflows or navigating deep directory structures, **dosh** is built to enhance your productivity without the overhead of traditional shells.

---

## ✨ Features

### 🛠️ Professional-Grade Built-in Commands
**dosh** features a massive suite of internal Unix-like utilities rewritten in Rust for maximum efficiency. These commands work identically across all operating systems.

* **Filesystem Management:** Navigate and organize with ease using `cd`, `pwd`, `ls`, `mkdir`, `rmdir`, `touch`, `cp`, `mv`, `rm`, `find`, `stat`, `file`, `tree`, `chmod`, `chown`, and `ln`.
* **Text Processing & Manipulation:** Advanced data handling with `cat`, `head`, `tail`, `grep` (with colored output), `sed`, `awk`, `cut`, `sort`, `uniq`, `wc`, `diff`, `cmp`, `tee`, and `xargs`.
* **Process & System Control:** Full visibility into your machine via `ps`, `top`, `htop`, `kill`, `killall`, `jobs`, `fg`, `bg`, `wait`, `uptime`, `who`, `whoami`, `id`, `uname`, `df`, `du`, and `free`.
* **Networking & Remote Access:** Built-in connectivity tools including `ping`, `curl`, `wget`, `scp`, `ssh`, and `ftp`.
* **Compression & Archival:** Handle packages natively with `tar`, `zip`, `unzip`, `gzip`, and `gunzip`.
* **Scripting & Environment:** Manage your workspace with `history`, `alias`, `unalias`, `export`, `env`, `set`, `unset`, `source`, `clear`, `sleep`, `date`, `time`, and `test` (including `[[ ]]` support).

### 🪟 Native PowerShell Integration
For users on Windows, **dosh** acts as a powerful wrapper. It allows you to invoke critical PowerShell cmdlets directly, ensuring you don't lose access to system-specific management tools.
* **Supported Cmdlets:** Seamlessly run `Get-ChildItem`, `Set-Location`, `Get-Location`, `New-Item`, `Remove-Item`, and more.
* **Why it matters:** This integration allows Windows developers to use Unix logic (like `&&` chaining) while still interacting with the Windows Management Instrumentation (WMI) and the Registry via PowerShell.

### 🎨 Intelligent Segmented Prompt
Efficiency is driven by information. The **dosh** prompt is split into high-visibility segments to keep you informed without running extra commands.
* **Left Segments:** `[User]` @ `[Hostname]` : `[Current Directory]`
* **Right Segments:** `[Git Branch/Status]` | `[Language Runtime (Node/Rust/Python)]` | `[HH:MM:SS]`
* **Visual Clarity:** Color-coded status indicators change when a command fails or when a Git repository has unstaged changes, significantly reducing cognitive load.

### 🧠 Persistent UX & History
Never lose a complex command again. 
* **Automatic Persistence:** Your history is saved instantly across all open sessions.
* **Advanced Search:** Use fuzzy matching to find that one specific `curl` command you ran three weeks ago.

### 🔄 Integrated Self-Update
Stay on the cutting edge without manual maintenance. 
* **GitHub Sync:** **dosh** automatically checks the official GitHub releases for new versions.
* **One-Click Upgrade:** If an update is found, the shell prompts you to download and install it, handling the binary replacement automatically.

### 🦀 The Rust Advantage
Because **dosh** is built in Rust, it inherits:
* **Speed:** Zero-cost abstractions and highly optimized crates make command execution near-instant.
* **Safety:** No data races or memory leaks, ensuring your shell remains stable during 24/7 uptimes.
* **Concurrency:** Heavy tasks like `grep` or `find` utilize multi-threading to scan large filesystems in a fraction of the time.

---

## 📦 Installation

### Pre-built Binaries
1. Go to the [GitHub Releases](https://github.com/username/dosh/releases) page.
2. Download the executable for your architecture (`windows`, `linux`, or `macos`).
3. Add the binary to your system's `PATH`.

### Build from Source
Ensure you have the latest [Rust toolchain](https://rustup.rs/) installed.

```bash
# Option 1: Install directly from crates.io
cargo install dosh
```
# Option 2: Clone and build
git clone [https://github.com/username/dosh.git](https://github.com/username/dosh.git)
cd dosh
cargo build --release
## 🚀 Usage

**dosh** supports advanced command chaining, logical operators, and intelligent completion to streamline your workflow.

### Examples
* **Chaining:** Use `&&` for conditional execution or `;` for sequential tasks.
    * `mkdir assets && cd assets ; touch styles.css`
* **Git Awareness:** Navigate to any repository to see the prompt transform, providing real-time branch names and dirty/clean status indicators.
* **Tab Completion:** Simply press `Tab` to see smart, context-aware suggestions for commands, local file paths, or environment variables.

### Command Reference

| Command | Usage |
| :--- | :--- |
| `grep <pattern> <file>` | Search for text within a file with automatic syntax highlighting. |
| `history` | List all previously executed commands with searchable IDs. |
| `update` | Manually trigger a check for the latest GitHub release. |
| `export VAR=VAL` | Set environment variables for the duration of the current session. |
| `[[ -f file ]]` | Perform advanced file existence and string equality testing. |
| `exit` | Securely close the shell session and save all history data. |

---

## ⚙️ Configuration

**dosh** is designed to be low-maintenance, storing state and configuration in the following default system locations:

* **History File:** `~/.dosh_history` (or `%USERPROFILE%\.dosh_history` on Windows).
* **Update Cache:** `~/.dosh/updates/` — stores temporary files during the self-update process.
* **Configuration:** Custom aliases and prompt settings can be modified in `~/.doshrc` *(Feature coming soon)*.

---

## 🤝 Contributing

We welcome and appreciate contributions from the community! To get involved:

1.  **Report Issues:** Encounter a bug? Open a ticket on our [Issue Tracker](https://github.com/username/dosh/issues).
2.  **Submit PRs:** Fork the repository, create a descriptive feature branch, and ensure your code passes `cargo test`.
3.  **Coding Standards:** To maintain high code quality, we strictly adhere to `rustfmt` for formatting and `clippy` for linting.

---

## 📄 License

This project is licensed under the **MIT License**. For more details, please see the [LICENSE](https://github.com/username/dosh/blob/main/LICENSE) file in the repository.

* **GitHub Repository:** [https://github.com/username/dosh](https://github.com/username/dosh)
* **Releases:** [View all versions](https://github.com/username/dosh/releases)