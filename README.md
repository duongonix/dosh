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


### Dosh cheatsheet

This section is a practical, command-first index of what Dosh can do today.

## 1. String operations
```bash
"hello" | upper
"HELLO" | lower
"hello world" | title
" hello " | trim
" hello" | trim-start
"hello " | trim-end
"hello world" | capitalize
"hello world" | reverse
"hello world" | length
"hello world" | words
"hello\nworld" | lines
"hello world" | split " "
["hello", "world"] | join " "
"hello world" | slice 0 5
"hello world" | contains "world"
"hello world" | starts-with "hello"
"hello world" | ends-with "world"
"hello world" | replace "world" "dosh"
"abc123" | extract "\\d+"
"abc123" | match "\\d+"
"hello" | repeat 3
"hello" | pad-left 10
"hello" | pad-right 10
"user@example.com" | is-email
"https://dosh.dev" | is-url
"42" | to-int
"3.14" | to-float
"true" | to-bool
"{\"name\":\"dosh\"}" | from-json
```

## 2. Number and units
```bash
10 | add 5
10 | sub 3
10 | mul 2
10 | div 2
10 | mod 3
-10 | abs
2 | pow 8
16 | sqrt
3.14159 | round 2
3.9 | floor
3.1 | ceil
120 | clamp 0 100

10 | gt 5
10 | gte 10
10 | lt 20
10 | lte 10
10 | eq 10
10 | neq 5
10 | is-even
11 | is-odd

1mb | to kb
1gb | to mb
1hr | to min
90sec | to min
```

## 3. List / array
```bash
[1, 2, 3] | length
[1, 2, 3] | first
[1, 2, 3] | last
[1, 2, 3] | take 2
[1, 2, 3] | skip 1
[1, 2, 3] | reverse

[1, 2, 3] | append 4
[1, 2, 3] | prepend 0
[1, 2, 3] | insert-at 1 99
[1, 2, 3] | remove-at 1
[1, 2, 3, 4] | slice 1 3
[1, 2, 2, 3] | unique
[3, 1, 2] | sort
[[1, 2], [3, 4]] | flatten
[1, 2, 3] | contains 2

[1, 2, 3] | map { $it * 2 }
[1, 2, 3] | filter { $it > 1 }
[1, 2, 3] | reduce 0 { $acc + $it }

[1, 2, 3] | sum
[1, 2, 3] | avg
[1, 2, 3] | min
[1, 2, 3] | max
[1, 2, 3] | count
```

## 4. Record / object
```bash
{name: "dosh", age: 1} | get name
{name: "dosh", age: 1} | select name
{name: "dosh", age: 1} | reject age

{name: "dosh"} | insert age 1
{name: "dosh", age: 1} | update age 2
{name: "dosh"} | rename name title
{name: "dosh", age: 1} | drop age

{name: "dosh", age: 1} | keys
{name: "dosh", age: 1} | values
{name: "dosh"} | has name
{name: "dosh"} | merge {age: 1}

{name: "dosh", meta: {lang: "rust"}} | get meta.lang
{name: "dosh", meta: {lang: "rust"}} | update meta.lang "dosh"

{name: "dosh"} | to-json
{name: "dosh"} | table
```

## 5. Table / list of records
```bash
[
  {name: "a", age: 20},
  {name: "b", age: 30}
] | where age > 20

[
  {name: "a", age: 20},
  {name: "b", age: 30}
] | select name

[
  {name: "a", age: 20},
  {name: "b", age: 30}
] | reject age

[
  {name: "a", age: 20},
  {name: "b", age: 30}
] | sort-by age

[
  {country: "VN", name: "a"},
  {country: "VN", name: "b"},
  {country: "TW", name: "c"}
] | group-by country

[
  {name: "a"},
  {name: "b"}
] | update status "active"

[
  {name: "a"},
  {name: "b"}
] | insert id { $index }

[
  {name: "a", old: 1}
] | rename old new

[
  {name: "a", age: 20}
] | first | get name
```

## 6. Core shell builtins
```bash
pwd
cd src
cd ..
echo hello
clear
history
alias ll = ls
unalias ll
source ./startup.dosh
help
help where
exit
```

## 7. Environment builtins
```bash
env
env | get PATH
export API_KEY "abc"
unset API_KEY
path
which cargo
whereis git
```

## 8. Filesystem and file transforms
```bash
ls
ls -la
ls -R
ls | where size > 1mb
ls | sort-by modified | last
ls | select name size modified

mkdir build
touch notes.txt
cat notes.txt
head notes.txt
tail notes.txt
stat Cargo.toml
du .
find . | where name contains "test"

rm temp.txt
rm -r target/tmp
rm -rf target/tmp
cp README.md README.bak
cp -r src src_bak
mv old.txt new.txt
```

## 9. Glob, open, edit, open-each, save
```bash
glob "**/*.rs"
glob "**/*.{rs,toml}" --depth 2
glob "src/**/*.rs" --absolute

open package.json
open config.toml
open users.csv
open README.md --raw

open package.json | get scripts
open package.json | update scripts.dev "vite --host" | save --in-place

open users.json | select name email | save users.csv
open users.csv | where age > 18 | save adults.csv
open data.yaml | to-json | save data.json
open data.json | to-yaml | save data.yaml

"hello" | save ok.txt
[{a: 1}] | save ok.json
{name: "dosh"} | save config.toml
"hello" | save log.txt --append

files "**/*.md"            # legacy mention only; use glob in current naming
edit "src/**/*.rs"
edit "**/*.json" | update app.name "Dosh" | save --in-place --backup

glob "**/*.md" | open-each --raw | replace "DoShell" "Dosh" | save --in-place --backup
glob "src/**/*.rs" | open-each --raw | replace --regex "old_(.*?)" "new_$1" | diff
```

## 10. Save safety options
```bash
open a.json | update app.name "Dosh" | save --in-place
open a.json | update app.name "Dosh" | save --in-place --backup
open a.json | update app.name "Dosh" | save --in-place --dry-run
open a.json | update app.name "Dosh" | preview
open a.json | update app.name "Dosh" | diff
```

## 11. Structured query and transform builtins
```bash
get scripts
get user.email
get users.0.name

select name age
reject password token
where size > 100kb
where name == "main.rs"
filter age >= 18
sort-by modified
group-by country

count
length
first
last
take 10
skip 5
reverse
flatten

insert status "active"
update status "inactive"
rename old_name new_name
drop debug
keys
values

each {|it| $it.name }
map { $it * 2 }
reduce 0 { $acc + $it }
```

## 12. Format conversion builtins
```bash
"{\"a\":1}" | from-json
{a: 1} | to-json

"a: 1" | from-yaml
{a: 1} | to-yaml

"a = 1" | from-toml
{a: 1} | to-toml

"name,age\nalice,30" | from-csv
[{name: "alice", age: 30}] | to-csv
```

## 13. Process / jobs
```bash
ps
ps | where name contains "node"
ps | sort-by memory | last
ps | select pid name cpu memory

spawn cargo test
jobs
wait
kill 1234
fg 1234
bg 1234
```

## 14. Network / durl
```bash
durl https://api.example.com
durl get https://api.example.com/users
durl get https://api.example.com/users --full
durl get https://example.com --raw

durl get https://api.example.com/users --query {"page":1,"limit":20}
durl get https://api.example.com -H "Accept: application/json"
durl get https://api.example.com --bearer $TOKEN
durl get https://api.example.com --basic admin password

{name: "dosh"} | durl post https://api.example.com/projects
durl post https://api.example.com/users --json {"name":"donix","age":20}
durl post https://api.example.com/login --form {"username":"admin","password":"123"}

durl get https://example.com/file.zip --output file.zip
durl get https://example.com/file.zip | save file.zip
```

## 15. External command bridge
```bash
^git status
^cargo test | complete
^cargo test | complete | get exit_code
^cargo test | complete | get stderr
^git branch | lines | where contains "*"

$tool = "C:\\Program Files\\Git\\bin\\git.exe"
^$tool status
```

## 16. Watch and reactive pipeline foundations
```bash
watch .
watch src --glob "**/*.rs"
watch . --glob "**/*.rs" --duration 2000

watch src --glob "**/*.rs" | debounce 500
watch src --glob "**/*.rs" | throttle 500
watch src --glob "**/*.rs" | changed-files

watch src --glob "**/*.rs" | run cargo test
```

## 17. Prompt and theme commands
```bash
prompt show
prompt segments
prompt doctor
prompt theme classic
prompt reload
prompt preview minimal
```

## 18. Plugin management commands
```bash
plugin init --name hello-http
plugin install --from ./hello-http
plugin list
plugin enable hello-http
plugin disable hello-http
plugin remove hello-http

plugin trust add --id org-key --public-key <base64>
plugin trust list
plugin trust remove --id org-key
```

## 19. Script language examples
```bash
$name = "dosh"
$user = {name: "duong", age: 20}
$user.name = "donix"

fn greet($x) { echo $x }
greet "hello"

if $name == "dosh" { echo ok } else { echo no }
for $i in [1,2,3] { echo $i }
match 2 { 1 => { echo one }; _ => { echo other } }

test "basic math" { assert eq 1 1 }
```

## 20. Practical end-to-end pipelines
```bash
open users.json
| where age >= 18
| update status "adult"
| sort-by name
| select name email status
| save adults.csv

glob "src/**/*.rs"
| where size > 10kb
| sort-by size
| take 10
| select path size
| table

"hello world"
| split " "
| map { $it | title }
| join " "

open package.json
| get scripts
| keys
| sort
| table

open access.log --raw
| lines
| where contains "500"
| count
```

## 21. What this cheatsheet gives users

If a user reads this cheatsheet, they should understand:

- Dosh can process strings, numbers, lists, records, and tables as typed data.
- Dosh pipelines are not text-only.
- Dosh supports safe file transformations and bulk edits.
- Dosh supports structured HTTP workflows (`durl`) and plugin extension.
- Dosh has script language foundations and reactive runtime foundations.

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
