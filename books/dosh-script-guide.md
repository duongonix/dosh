# Dosh Script Guide

This guide helps you write real Dosh scripts from zero to production workflows.

## 1. Script Philosophy

Dosh script is:

- shell-first
- pipeline-first
- structured-data-first

Instead of treating everything as plain text, Dosh keeps values typed across pipelines.

## 2. Run Script

Run file:

```dosh
dosh run ./script.dosh
```

Validate parse:

```dosh
dosh check ./script.dosh
```

Run tests inside scripts:

```dosh
dosh test
```

## 3. Variables and Constants

Assignment uses `$name = ...`:

```dosh
$name = "dosh"
$age = 1
$user = { name: "duong", role: "dev" }
```

Nested assignment:

```dosh
$user.name = "donix"
$user.meta = { lang: "rust" }
$user.meta.lang = "dosh"
```

Uppercase names are treated as immutable constants by convention:

```dosh
$API_URL = "https://api.example.com"
```

## 4. Value Types You Use in Script

- `string`, `int`, `float`, `bool`, `null`
- `duration` (`1sec`, `500ms`, `1hr`)
- `filesize` (`1kb`, `10mb`, `1gb`)
- `list`
- `record`
- `table`

Examples:

```dosh
$size = 10mb
$timeout = 3sec
$users = [
  { name: "a", age: 20 }
  { name: "b", age: 30 }
]
```

## 5. String Interpolation

Use interpolated string:

```dosh
$name = "dosh"
print $"hello ($name)"
```

Useful for paths:

```dosh
$path = $"($HOME)/.config/dosh/commands/devflow.dosh"
print $path
```

## 6. Functions

Define function:

```dosh
fn greet($name) {
  print $"Hello ($name)"
}
```

Return value:

```dosh
fn add($a, $b) {
  $a + $b
}
```

Call:

```dosh
greet "duong"
add 2 3
```

## 7. Control Flow

If/else:

```dosh
if $env == "prod" {
  print "production mode"
} else {
  print "dev mode"
}
```

For:

```dosh
for $x in [1,2,3] {
  print $x
}
```

Match:

```dosh
match $action {
  "build" => { print "build task" }
  "test" => { print "test task" }
  _ => { print "unknown" }
}
```

## 8. Structured Data Access

Cell path access:

```dosh
$cfg = { server: { host: "localhost", port: 3000 } }
$cfg.server.host
$cfg.server.port
```

List index path:

```dosh
$users = [{name: "a"}, {name: "b"}]
$users.0.name
```

## 9. Pipelines in Script

You can write same interactive pipelines inside script:

```dosh
open package.json | get scripts | keys | sort | table
```

Store pipeline result:

```dosh
$adults = open users.json | where age >= 18 | update status "adult"
$adults | save adults.csv
```

## 10. Closures and Transform Style

Map:

```dosh
[1,2,3] | map { $it * 2 }
```

Filter:

```dosh
[1,2,3,4] | filter { $it > 2 }
```

Reduce:

```dosh
[1,2,3] | reduce 0 { $acc + $it }
```

## 11. External Command Bridge

Run explicit external:

```dosh
^git status
```

String path as external:

```dosh
$tool = "C:\\Program Files\\Git\\bin\\git.exe"
^$tool --version
```

Capture process output:

```dosh
^cargo test | complete | get exit_code
```

## 12. Modules and File Organization

Recommended:

- `commands/` for user commands
- `modules/` for reusable helpers

Example module:

```dosh
mod util {
  export fn slug($s) {
    $s | lower | replace " " "-"
  }
}
```

## 13. Error Handling Pattern

Common safe pattern:

```dosh
if (glob "src/**/*.rs" | count) == 0 {
  print "no rust files found"
} else {
  glob "src/**/*.rs" | table
}
```

When doing destructive operations, prefer:

```dosh
edit "**/*.json"
| update app.name "Dosh"
| save --in-place --dry-run
```

Then run with `--backup`.

## 14. Script Testing

Create test blocks:

```dosh
test "sum basic" {
  assert eq ([1,2,3] | sum) 6
}
```

Run:

```dosh
dosh test
```

## 15. Practical Script Example

`sync-users.dosh`:

```dosh
$input = "users.json"
$output = "adults.csv"

open $input
| where age >= 18
| update status "adult"
| sort-by name
| select name email status
| save $output

print $"done: ($output)"
```

## 16. Best Practices

- Keep pipelines structured, avoid converting to text too early.
- Use `--dry-run` for destructive writes.
- Use `table`/`inspect` to debug intermediate data.
- Put reusable logic in exported functions.
- Keep command scripts small and composable.

