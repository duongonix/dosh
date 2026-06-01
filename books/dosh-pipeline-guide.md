# Dosh Pipeline Guide

This guide teaches how to think and work with Dosh pipelines effectively.

## 1. Pipeline Philosophy

Classic shell:

- pipe passes text

Dosh:

- pipe passes structured values first

This means your command chains can query, transform, and save data without fragile text parsing.

## 2. Pipeline Shape

Use this model:

`source | transform | action`

Examples:

```dosh
open package.json | get scripts
ls | where size > 1mb | sort-by modified
glob "**/*.md" | open-each --raw | replace "DoShell" "Dosh" | save --in-place --backup
```

## 3. Core Pipeline Data Types

Pipelines can carry:

- string
- int/float/bool
- duration/filesize
- list
- record
- table
- text from external command bridge

## 4. Pipeline Sources

Main sources:

- `open` (json/yaml/toml/csv/text)
- `ls` (table)
- `glob` (table of files)
- `open-each` (per-file value stream foundation)
- literals:
  - `"hello"`
  - `{name: "dosh"}`
  - `[{name: "a"}]`

Examples:

```dosh
"hello" | save hello.txt
{name:"dosh"} | to-json
[{name:"a",age:20}] | select name
```

## 5. Transform Commands

Data transforms:

- `get`, `select`, `reject`, `where`, `filter`
- `sort-by`, `group-by`
- `insert`, `update`, `rename`, `drop`
- `keys`, `values`
- `first`, `last`, `take`, `skip`, `count`, `length`
- `map`, `each`, `reduce`

Text transforms:

- `replace`
- `split`
- `lines`
- `trim`

## 6. Action Commands

Actions usually end pipeline:

- `save`
- `table`
- `diff`
- `preview`
- `rm` (when used on file records/path flows)

## 7. Structured Query Examples

```dosh
open users.json | where age >= 18 | select name email
open package.json | get scripts | keys | sort | table
ls | where type == "file" | where size > 100kb | select name size modified
```

## 8. File Transformation Pipelines

Single file:

```dosh
open package.json
| update scripts.dev "vite --host"
| save --in-place --backup
```

Bulk files:

```dosh
glob "**/*.md"
| open-each --raw
| replace "DoShell" "Dosh"
| save --in-place --backup
```

Dry-run safety:

```dosh
glob "**/*.json"
| open-each
| update app.name "Dosh"
| save --in-place --dry-run
```

## 9. External Command Bridge in Pipeline

Use `^` for external command:

```dosh
^git status
^cargo test | complete | get exit_code
^git branch | lines | where contains "*"
```

`complete` gives structured output (`stdout`, `stderr`, `exit_code`, `duration`).

## 10. Mixed Structured/Text Pipelines

Example:

```dosh
^npm run | lines | where contains "dev"
```

Here:

- `^npm run` -> text
- `lines` -> list of string
- `where contains "dev"` -> filtered list

## 11. Cell Path Navigation in Pipelines

Examples:

```dosh
open package.json | get scripts.dev
open users.json | get users.0.email
```

Use dotted paths for nested record/list access.

## 12. Unit-aware Filtering

Dosh supports filesize/duration literals:

```dosh
ls | where size > 1mb
ls | where size <= 512kb
```

Convert units:

```dosh
1gb | to mb
90sec | to min
```

## 13. Inspect and Debug Pipelines

When result looks wrong:

```dosh
... | inspect
... | table
```

You can also split long chain into checkpoints:

```dosh
$x = open users.json | where age >= 18
$x | inspect
$x | select name email | table
```

## 14. Safety Patterns

For destructive workflows:

1. run with `--dry-run`
2. inspect/diff
3. run with `--backup`

Example:

```dosh
edit "src/**/*.rs"
| replace --regex "old_(.*?)" "new_$1"
| diff
```

Then:

```dosh
edit "src/**/*.rs"
| replace --regex "old_(.*?)" "new_$1"
| save --in-place --backup
```

## 15. Reactive Pipeline Foundation

Watch source + transforms:

```dosh
watch src --glob "**/*.rs"
| debounce 500ms
| changed-files
```

Use this for automation flows and CI-like local feedback loops.

## 16. Performance Tips

- Filter early (`where` near source).
- Select only needed columns (`select`).
- Avoid repeated parse/serialize conversions.
- For large delete, use `rm --fast`.
- For huge file batches, start with dry-run to estimate scope.

## 17. Common Mistakes

### No output

- Input type mismatch with command.
- Missing `table` at the very end in some debugging contexts.
- Filter condition too strict.

### `program not found`

- forgot `^` for external command
- typo in builtin/alias/plugin command

### parse error with records

Use valid record syntax:

```dosh
{name:"dosh"} | save d.json
```

## 18. Practical End-to-End Pipelines

JSON -> CSV:

```dosh
open users.json
| where active == true
| select id name email
| save active_users.csv
```

Project scan report:

```dosh
glob "src/**/*.rs"
| where size > 10kb
| sort-by size
| take 10
| select path size modified
| table
```

Filesystem cleanup target list:

```dosh
ls
| where type == "dir"
| where name contains "target"
| table
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
rm node_modules --fast
rm dist --dry-run
rm build --trash
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
