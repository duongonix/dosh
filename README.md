# Dosh Shell: Full Product & Architecture Overview

## 1. What Dosh Is

Dosh is a **modern structured-data shell** write in Rust designed with an even stronger architecture-first direction:

- shell runtime
- scripting language
- structured data pipeline engine
- plugin runtime (WASM-first foundation)
- reactive automation foundation
- developer operating environment foundation

Unlike classic shells that treat everything as plain text, Dosh treats data as typed values and passes structured objects through pipelines.

---

![Dosh](/assets/demo.png)

## Install

### Quick install (latest)

Linux/macOS:
```bash
curl -fsSL https://raw.githubusercontent.com/duongonix/dosh/main/scripts/install.sh | sh
```

Windows (PowerShell):
```powershell
iwr -useb https://raw.githubusercontent.com/duongonix/dosh/main/scripts/install.ps1 | iex
```

### Install specific version

Linux/macOS:
```bash
curl -fsSL https://raw.githubusercontent.com/duongonix/dosh/main/scripts/install.sh | sh -s -- v1.0.0
```

Windows (PowerShell):
```powershell
& ([scriptblock]::Create((iwr -useb https://raw.githubusercontent.com/duongonix/dosh/main/scripts/install.ps1))) -Version v1.0.0
```

### Verify

```bash
dosh --version
durl --version
```

### Update Dosh

```bash
dosh update
```

`dosh update` checks latest GitHub release, prompts for confirmation, downloads the correct asset for your OS/arch, verifies SHA256 (when checksum asset exists), replaces the current binary, and asks you to restart shell.


## 2. Core Philosophy

### Bash/Zsh model
- Everything is text.
- Pipelines pass stdout/stderr strings.

### Dosh model
- Everything is structured data first.
- Pipelines pass typed values (`Value`, `Table`, stream foundations), with text compatibility for external commands.

### Design principles
- Extensible architecture first
- Cross-platform (Windows/macOS/Linux)
- Async-ready / stream-ready foundation
- Safe-by-default for destructive workflows
- Clean modular crates (avoid monolithic files)

---

## 3. Workspace Architecture (High Level)

Dosh is split across multiple crates/modules, including (non-exhaustive):

- `dosh-cli`: CLI entrypoint, subcommands, plugin management commands
- `dosh-core`: shell app layer, REPL integration, error rendering
- `dosh-runtime`: AST execution, pipelines, command dispatch, plugin runtime bridge
- `dosh-parser`: script/parser (commands, pipelines, literals, functions, control flow)
- `dosh-ast`: language AST types
- `dosh-value`: typed value system
- `dosh-builtins`: builtin registry and command implementations
- `dosh-config`: path/config conventions
- `dosh-plugin`: plugin manifest, install/state/trust/permissions
- `dosh-wasm`: WASM plugin execution runtime
- `dosh-prompt`: prompt engine and themes
- `dosh-history`: persistent command history foundation
- `dosh-completion`: completion foundation
- `dosh-highlight`: syntax highlighting foundation
- `dosh-durl` / `durl-core` / `durl-cli`: structured HTTP client stack
- `dosh-dedit`: terminal editor package integrated as builtin

---

## 4. Language & Script Model

Dosh script supports:

- variable assignment with `$var = ...`
- constants via uppercase names (`$NAME = ...`, no reassignment)
- nested assignment paths (`$user.name = "x"`)
- functions (`fn ...`)
- `return`, `if / elif / else`, `for`, `match`
- modules/import/export foundations
- test blocks (`test "..." { ... }`)
- pipeline expressions
- closures/lambda foundations used in data commands (`each`, `map`, `reduce`, etc.)
- interpolation in command args (`$var` usage in command tokens)

---

## 5. Typed Value System

Dosh supports structured typed values (core foundation):

- `Null`
- `Bool`
- `Int`
- `Float`
- `String`
- `Duration`
- `Filesize`
- `DateTime` foundation
- `Binary`
- `List`
- `Record`
- `Table`
- stream foundation

Capabilities include:

- serde serialization/deserialization
- pretty display helpers
- type inspection (`type_name`-style behavior)
- truthiness checks
- nested field/cell-path access
- list/table record navigation
- filesize parsing (`b`, `kb`, `mb`, `gb`, `tb`)
- duration parsing (`ms`, `sec`, `min`, `hr`, `day`)

---

## 6. Pipeline System

Dosh pipeline architecture supports:

- text pipeline
- structured pipeline
- mixed text/structured boundary
- external command interop
- stream/lazy foundation

Key runtime abstractions:

- `PipelineData`
- stream wrappers/foundations
- runtime context
- command input/output boundaries
- exit status abstraction
- pipeline error diagnostics

Examples:

- `ls | where size <= 1mb | sort-by modified`
- `open package.json | get scripts`
- `ps | where memory > 100mb | sort-by cpu`
- `durl get https://api.example.com/users | where active == true`

---

## 7. Builtin Registry & Help

Dosh has a builtin registry architecture with metadata support:

- command name
- usage
- description
- examples
- help integration (`help`, `help <command>`)

Builtins are registered through modular factory/registry patterns (plugin-compatible direction).

---

## 8. Builtins: Feature Inventory

Note: Dosh has a large and evolving builtin surface. This list captures the implemented/foundation command families currently present in project direction.

### 8.1 Core Shell
- `cd`
- `pwd`
- `exit`
- `clear`
- `echo`
- `help`
- `history`
- `alias`
- `unalias`
- `source`

### 8.2 Environment
- `env`
- `export`
- `unset`
- `path`
- `which`
- `whereis`

### 8.3 Filesystem & File/Data Workflows
- `ls` (structured output, table rendering)
- `open` (json/yaml/toml/csv/text + foundations)
- `mkdir`
- `rm` (safe foundations, recursive/force capabilities)
- `cp`
- `mv`
- `touch`
- `cat`
- `head`
- `tail`
- `du`
- `stat`
- `find`
- `glob` (renamed from `files`)
- `edit` (glob + open-each shortcut)
- `open-each`
- `save` (path/in-place, backup, dry-run, append, format inference)
- `replace` (text/regex/path/recursive)
- `diff`
- `preview`
- `confirm` foundation
- `watch` (reactive file event foundation)

### 8.4 Structured Data Commands
- `table`
- `inspect`
- `get`
- `select`
- `reject`
- `where`
- `filter`
- `each`
- `map`
- `reduce`
- `sort-by`
- `group-by`
- `count`
- `length`
- `first`
- `last`
- `skip`
- `take`
- `reverse`
- `flatten`
- `transpose` foundation
- `merge`
- `join` foundation
- `insert`
- `update`
- `rename`
- `drop`
- `keys`
- `values`

### 8.5 Format Conversion
- `from-json` / `to-json`
- `from-yaml` / `to-yaml`
- `from-toml` / `to-toml`
- `from-csv` / `to-csv`
- `from-xml` / `to-xml` foundation
- `from-ini` / `to-ini` foundation

### 8.6 String/Text
- `str` command family foundations
- `split`
- `lines`
- `trim`
- `replace`
- `contains`
- `starts-with`
- `ends-with`
- `match`
- `parse`
- `format`
- plus direct pipeline-style string/list operations in structured commands

### 8.7 Math
- `math`
- `sum`
- `avg`
- `min`
- `max`
- `median` foundation
- `round`
- `floor`
- `ceil`
- `random`

### 8.8 Date/Time
- `date`
- `now`
- `format-date`
- `parse-date`
- `sleep`
- `timer`

### 8.9 Process/Jobs
- `ps`
- `kill`
- `jobs`
- `fg`
- `bg`
- `spawn`
- `wait`

### 8.10 Network
- `http` foundation
- `fetch` foundation
- `curl` foundation
- `ping` foundation
- `dns` foundation
- `port`
- `serve` foundation
- `durl` (structured HTTP client stack)

### 8.11 System Info
- `sys`
- `cpu`
- `mem`
- `disk`
- `battery` foundation
- `os`
- `hostname`
- `whoami`
- `uptime` foundation

### 8.12 Developer / Workflow
- `run`
- `task` foundation
- `doctor` foundation
- `bench` foundation
- `test` integration foundations
- package/git related foundations

### 8.13 Security / Safety
- `confirm`
- `secret` foundation
- `hash` (sha256 baseline)
- `encrypt` foundation
- `decrypt` foundation
- `permissions` foundation

### 8.14 Pipeline / Runtime Introspection
- `pipeline inspect` foundation
- `pipeline trace` foundation
- `inspect` command for structured data shape/sample

### 8.15 Undo/Action Log Foundations
- `undo` foundation
- `redo` foundation
- action log architecture for reversible filesystem workflows (foundation)

### 8.16 SQL / DataFrame Foundations
- `sql` foundation
- SQLite/XLSX integration points in file pipeline architecture

---

## 9. Durl (Structured HTTP Client)

Durl direction:

- curl-like ergonomics
- structured output first
- pipeline-native body and response flows

Capabilities/foundations:

- methods: GET/POST/PUT/PATCH/DELETE
- JSON auto-parse
- `--raw`
- `--full` response object
- headers/query/body/auth flags
- timeout/retry/follow foundations
- output/download support
- integration with Dosh structured pipelines

---

## 10. File Transformation Pipeline Model

Dosh supports the `source | transform | action` pattern for file/data operations.

### Sources
- `open`
- `glob`
- `edit`
- `open-each`

### Transforms
- `replace`
- `update`
- `where`
- `select`
- `map/each/reduce`
- sort/group/get/insert/rename/remove-style operations

### Actions
- `save`
- `diff`
- `preview`
- `confirm`
- undo/rollback foundations

Typical examples:

- `open file.json | update app.name "Dosh" | save --in-place`
- `glob "**/*.md" | open-each --raw | replace "DoShell" "Dosh" | save --in-place --backup`

---

## 11. External Command Bridge

Dosh supports external commands alongside builtins:

- standard command execution
- explicit external prefix `^` (including string/variable command paths)
- structured capture patterns (`complete`-style foundation)
- stdout/stderr/exit metadata foundations

Goal: keep shell compatibility while maintaining structured internals.

---

## 12. Prompt System

Dosh prompt architecture (product direction):

- context-driven prompt engine
- theme-driven rendering (not hardcoded UI)
- segment registry
- smart context (cwd, git, runtime hints)
- right prompt and multiline foundations
- powerline/classic theme variants
- plugin segment extension foundation

---

## 13. Plugin System

### Plugin model
- manifest-driven plugin packages
- command registration from manifest
- permission declarations
- trust/signing foundations
- install/list/enable/disable/remove management commands
- WASM runtime execution (`dosh-wasm`)

### Current behavior notes
- Runtime expects plugin ABI exports:
  - `alloc`
  - `dealloc`
  - `dosh_run`
- Installer now validates/builds plugin wasm to avoid empty/stale module installs.

### Plugin path direction
- Unified plugin root:
  - `~/.config/dosh/plugins`

---

## 14. Error Diagnostics

Dosh error UX includes:

- readable error chain (`caused by`)
- contextual hints (`hint: ...`) in English
- parser diagnostics with statement context
- command-not-found hints and guidance

---

## 15. Cross-Platform Focus

Dosh is designed for:

- Windows
- macOS
- Linux

Key considerations:

- path separator handling
- executable resolution differences
- terminal rendering differences
- filesystem metadata differences
- process and background behavior differences
- plugin/runtime storage via path abstraction

---

## 16. Testing & Quality Direction

Project quality loop emphasizes:

- `cargo fmt`
- `cargo check --workspace`
- `cargo test --workspace`

Tests cover parser/runtime/value/query/builtins and integration foundations.

---

## 17. Strategic Direction

Dosh is evolving from a shell into a broader developer runtime:

- structured scripting language
- plugin ecosystem
- reactive automation runtime
- file/data transformation engine
- network/data workflows
- future DataFrame/SQL/AI integrations

In short:

**Dosh is not a Bash clone.**
It is a structured, extensible runtime for modern developer workflows.
