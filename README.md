# DAM — Project Continuity Platform

**DAM** is a modern developer platform and command-line tool for tracking, snapshotting, and restoring the state of your projects over time. Created by **oneam**, DAM is written primarily in Rust.

> ⚠️ **Project Status: Active Development (Alpha)**
>
> DAM is currently in active development. The architecture, command-line interface, and internal APIs continue to evolve. v0.6.0 represents a major step forward with enhanced project sharing, release management, and Git interoperability.

---

## 1. What is DAM?

DAM evolved from a standard Version Control System (VCS) into a comprehensive **Project Continuity Platform (PCP)**.

If you've ever wished you could:

* Save a checkpoint of your work
* Branch off to experiment safely
* Merge divergent work
* Restore a project to a known state
* Package an entire project for someone else
* Clone and import projects from network URLs just like Git
* Manage stable versions and releases independently

DAM is designed for exactly that.

Rather than focusing solely on tracking changes, DAM focuses on preserving the **continuity of a project**—its history, configuration, environment, files, and the context required to continue working on it.

---

## 2. The Problem DAM Solves

Traditional version control systems primarily ask:

> **"What changed?"**

DAM asks a broader question:

> **"How can this project be preserved, understood, recreated, and continued anywhere?"**

Maintaining project continuity often requires combining multiple disconnected tools for:

* Version history
* File collection rules
* Environment metadata
* Project setup
* Credential management
* Project packaging and distribution
* Release and version management
* Network-based project sharing

DAM brings these capabilities together into a single, cohesive platform, treating project continuity as critical infrastructure.

---

## 3. How DAM Differs

### Unique Architecture

DAM is not built on top of Git, nor is it intended to be a reskin of Git. It uses its own model for representing and preserving project state. However, it fully interoperates with Git repositories.

### First-Class Project Packaging

Beyond tracking files, DAM can export complete projects into `.dam` archives containing:

* Project metadata
* Environment setup commands
* Provider profiles
* Project state
* Pre-assigned releases and stable versions

### Strict Allowlisting

File collection is governed by `.purities` (allowlists) and `.impurities` (blocklists), with deterministic conflict resolution.

This replaces error-prone, exclusion-only approaches with explicit rules defining what is permitted to enter a collection.

### Integrated Secrets Management

DAM includes a built-in credential manager that can interface directly with:

* The operating system's Keychain
* A local AES-256-GCM encrypted vault

Sensitive credentials are therefore managed by DAM rather than being stored as plaintext configuration values or environment variables.

### Release & Stable Version Management

DAM separates concerns into three distinct tracking systems:

* **Seals** — Immutable snapshots of work in progress (stream-local)
* **Releases** — Named, distributable versions for external consumption
* **Stable Versions** — Stream-specific long-lived markers for compatibility

### Git-Compatible Import & Clone

Import projects directly from Git repositories or download `.dam` archives over HTTP/HTTPS, with full support for branching and Git-to-DAM conversions.

---

## 4. Core Terminology

DAM uses terminology inspired by water control systems. These terms are primary concepts within DAM and should not be treated as direct equivalents of concepts from other version control systems.

| Term           | Definition                                                                                                                 |
| -------------- | -------------------------------------------------------------------------------------------------------------------------- |
| **Reservoir**  | The totality of a project's continuity, including history, configuration, and state. Contained in the `.dam` folder.       |
| **Collection** | A holding pool of files selected to be saved in the next snapshot, governed by purity rules.                               |
| **Seal**       | An immutable, preserved historical snapshot of everything currently in the collection.                                     |
| **Timeline**   | The complete chronological history of seals on the current stream.                                                         |
| **Stream**     | An independent, parallel flow of development that can be switched between.                                                 |
| **Flowinto**   | The act of shifting the active workspace into a different stream.                                                          |
| **Apply**      | Restoring workspace files to match a specific seal.                                                                        |
| **Sync**       | Pushing or pulling seal history and file objects between a local reservoir and a cloud platform, such as GitHub.           |
| **Drain**      | Clearing a section of the reservoir, such as emptying the collection area.                                                 |
| **Export**     | Packaging a seal or an entire project into a portable archive for sharing.                                                 |
| **Release**    | A named, tagged snapshot of the project suitable for distribution and version tracking.                                    |
| **Stable**     | A stream-scoped version marker used to denote long-lived or release-ready branches.                                        |
| **Provider**   | A project-type profile, such as Flutter or Custom, that defines environment checks and setup commands for project exports. |

---

## 5. Architecture & the `.dam` Reservoir

At a high level, DAM operates on an immutable snapshot architecture.

When you initialize a project with `dam source`, DAM creates a hidden **`.dam` directory**, known as the **Reservoir**.

The Reservoir is the brain of the project and stores components such as:

* **Object Blobs** — The actual compressed data of tracked files.
* **Indices & Timelines** — A Directed Acyclic Graph (DAG) representing snapshot history.
* **Stream Metadata** — Information about parallel flows, including descriptions, goals, notes, and owners.
* **Releases** — Named version snapshots with metadata for distribution.
* **Stable Versions** — Stream-specific version markers.
* **Vault** — A secure `vault.bin` file when using the local encrypted credential manager instead of the OS Keychain.
* **Config** — Reservoir-level configuration such as `config.toml` and credential-related metadata.

---

## 6. Installation & Setup

### From Source

> **Note:** DAM is written primarily in Rust. Building from source is the standard installation method.

Ensure that [Rust and Cargo](https://rustup.rs/) are installed on your system.

Clone the repository and build the CLI:

```bash
git clone <dam-repo-url>
cd dam
cargo install --path .
```

You can verify the installation and check for available CLI updates with:

```bash
dam update
```

### Arch Linux (AUR)

DAM is available in the Arch User Repository:

```bash
yay -S dam
# or
paru -S dam
# or manually
git clone https://aur.archlinux.org/dam.git
cd dam
makepkg -si
```

---

## 7. Basic Usage

The following workflow demonstrates a standard DAM project lifecycle:

1. Initialize a reservoir
2. Define collection rules
3. Collect project state
4. Seal the collected state
5. View project history
6. Export and share

### 7.1 Initialize the Reservoir

Navigate to your project directory and run:

```bash
dam source
```

This launches an interactive setup wizard for configuring your project, including options such as:

* Project name
* Conflict resolution
* Provider profiles

Alternatively, initialize with a specific provider:

```bash
dam source --profile flutter
dam source -n "My App"
```

---

### 7.2 Define Purities

Create a `.purities` file to explicitly define which files may be collected.

DAM operates on strict allowlists by default.

Example:

```text
src/**
README.md
pubspec.yaml
```

Only files matching the configured purity rules are eligible for collection.

---

### 7.3 Collect State

Scan the current directory and stage matching files into the Collection:

```bash
dam collect .
```

Override rules on-the-fly:

```bash
dam collect . --rule-priority purities
dam collect . --override-impurities
```

---

### 7.4 Seal the State

Create an immutable snapshot of the currently collected environment:

```bash
dam seal "Established core project structure"
```

View recent seals:

```bash
dam seal --list 5
```

---

### 7.5 View History

View the project's historical timeline:

```bash
dam timeline --graph
```

The `--graph` option displays history as an ASCII topological graph.

---

## 8. Advanced Features

### 8.1 Streams and Safe Context Switching

DAM supports independent Streams for parallel development.

Create a new stream:

```bash
dam stream create feature-login
```

Switch the active workspace to that stream:

```bash
dam flowinto feature-login
```

If unsealed files are present when switching streams, DAM can prompt you to create a temporary **Continuity Snapshot** to help prevent accidental data loss.

Inspect a stream's metadata:

```bash
dam stream inspect feature-login
```

Manage stream continuity:

```bash
dam stream goal feature-login "Complete login UI"
dam stream notes feature-login "Using Firebase Auth"
dam stream description feature-login "Login feature branch"
```

---

### 8.2 Releases & Stable Versions

DAM separates **Releases** (distributable versions) from **Stable Versions** (stream-scoped markers).

#### Creating Releases

```bash
dam releases create v1.0.0
dam releases create v1.0.0 --latest
dam releases create v1.0.0 --tags stable,production --description "Production release"
```

List and inspect releases:

```bash
dam releases list
dam releases inspect v1.0.0
dam releases inspect --latest
```

#### Assigning Stable Versions

Stable versions mark long-lived or release-ready branches:

```bash
dam stable assign main v1.0.0 --latest
dam stable assign development v1.0.0-beta1 --description "Next release candidate"
```

List and inspect stable versions:

```bash
dam stable list
dam stable inspect --latest
dam stable remove v1.0.0
```

---

### 8.3 GitHub Synchronization

DAM can map local Streams directly to remote Git branches, such as:

```text
refs/heads/main
```

Authentication is handled through DAM's internal Credential Manager.

Push local stream history to the configured remote repository:

```bash
dam sync --action push
```

Pull remote changes:

```bash
dam sync --action pull
```

Sync releases intelligently:

```bash
dam sync --releases
```

---

### 8.4 Pull Request Workflows

DAM provides an interactive workflow for browsing open pull requests on a remote repository and checking them out into isolated local Streams.

For example:

```bash
dam pr checkout 19
```

This checks out Pull Request `#19` into a dedicated Stream named:

```text
pr-19
```

The active workspace is protected from unintended modification during the process.

List open pull requests:

```bash
dam pr list
```

---

### 8.5 Sealed File Integrity Protection

When pulling remote changes through operations such as `dam apply` or `dam pr checkout`, DAM verifies the hashes of files in the working directory against the latest Seal.

If unsealed modifications are detected, the operation is halted to protect local work from being accidentally overwritten.

---

### 8.6 Secure Credential Management

DAM includes a native Credential Manager for managing sensitive credentials such as:

* GitHub Fine-Grained Personal Access Tokens
* Classic Personal Access Tokens
* SSH keys
* Other provider credentials

Credentials can be stored using the operating system's Keychain or in a local AES-256-GCM encrypted vault.

Create a credential interactively:

```bash
dam creds create --alias github_token
dam creds create --vault  # Force local encrypted vault
```

List and delete credentials:

```bash
dam creds list
dam creds delete github_token
```

---

### 8.7 Exporting and Importing Projects

DAM can package a specific historical Seal or an entire project setup into a portable archive, with support for selective inclusion of releases and stable versions.

#### Project Exports with dam.toml

Project exports are configured via **`dam.toml`**, which defines:

* Project metadata (name, provider, etc.)
* Setup commands (automatic or manual review)
* Export profiles (full, contributor, viewer access)
* Named branches (streams) to include
* Pre-assigned releases and stable versions

Example `dam.toml`:

```toml
[project]
name = "my-flutter-app"
provider = "flutter"
enforce_password_on_project_import = false
description = "A Flutter mobile app"

[setup]
commands = [
    "flutter pub get",
    "flutter clean"
]

[profiles.full]
description = "Full Admin Access"
include = ["**/*"]
exclude = [".git/**", "build/**", ".dart_tool/**"]

[profiles.contributor]
description = "Contributor Access"
include = ["**/*"]
exclude = [".git/**", "build/**", ".dart_tool/**", "**/*.env"]

streams = ["main", "develop"]
releases = ["v1.0.0", "v1.1.0"]
stable = ["v1.0.0-stable"]
```

#### Export a Seal

Export a single historical snapshot:

```bash
dam export seal seal_001 --zip
```

#### Export a Project

Export the current workspace as a DAM project:

```bash
dam export project my-app
dam export project my-app --profile contributor
```

This packages:
* All project files matching the profile
* `dam.toml` with setup and export configuration
* Pre-assigned releases and stable versions
* Metadata and compression

#### Import a Project

Import a shared `.dam` archive:

```bash
dam import shared-app.dam
```

#### Clone & Import from Network

Clone a project directly from a Git repository:

```bash
dam import https://github.com/user/project.git
dam import git@github.com:user/project.git --branch develop
```

Import a `.dam` archive over HTTP/HTTPS:

```bash
dam import https://example.com/releases/my-app.dam
```

Merge cloned repository into current project:

```bash
dam import https://github.com/user/project.git --merge
```

---

### 8.8 Apply Shortcuts

Quickly apply seals without memorizing IDs:

```bash
dam apply --latest
dam apply --latest-global
dam apply seal_001 --preview
```

---

### 8.9 Merging

> ⚠️ **Experimental**
>
> DAM's merge functionality is currently experimental and should not be considered ready for production workflows.

DAM includes an experimental three-way conflict resolution engine designed to combine divergent Streams based on their closest common ancestor.

Example:

```bash
dam merge feature-login --apply
```

---

## 9. Configuration & Rule Files

DAM uses rule files to strictly control what `dam collect` is allowed to collect.

These files can be placed throughout the project tree.

### `.purities`

`.purities` files define **allowlists**.

A file must match an applicable purity rule to be eligible for collection.

Example:

```text
src/**
README.md
pubspec.yaml
analysis_options.yaml
```

---

### `.impurities`

`.impurities` files define **blocklists**.

They can be used to explicitly exclude files or directories from collection.

Example:

```text
build/
.env
.dart_tool/
*.log
```

Impurity rules inherit downwards through the project tree, allowing exclusions to be defined at the appropriate level.

Together, `.purities` and `.impurities` provide deterministic control over which project files become part of a Collection.

---

## 10. Provider Profiles

DAM includes built-in provider profiles that define environment checks and default setup commands:

### Flutter Provider

The Flutter provider checks for a valid Flutter installation and provides sensible defaults:

```bash
dam source --profile flutter
```

### Custom Provider

For non-standard project types:

```bash
dam source --profile custom
```

Custom providers bypass environment checks and rely on explicit setup commands.

---

## 11. Contributing

Contributions from early adopters are welcome.

Because DAM's architecture and internal APIs are actively evolving, please coordinate with the maintainer, **oneam**, before beginning major refactors or architectural changes.

For detailed instructions on:

* Setting up the development environment
* Running tests
* Contributing code
* Submitting pull requests

See the repository's `CONTRIBUTING.md`.

---

## 12. License

Please refer to the repository's `LICENSE` file for information about usage and distribution terms.

---