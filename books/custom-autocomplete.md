# Dosh Custom Autocomplete Guide

This guide explains how to build custom autocomplete in Dosh using Dosh script.

## 1. Philosophy

Dosh autocomplete is script-first:

- You define completion rules in `.dosh` files.
- You can return static values or compute dynamic values.
- Completion works with structured data and pipelines.

No hardcoded shell-specific syntax is required.

## 2. Where to Put Files

Use these directories:

- `~/.config/dosh/commands/` for custom commands (and completions in same file)
- `~/.config/dosh/completions/` for completion-only scripts
- `~/.config/dosh/modules/` for shared helpers

## 3. Completion Syntax

Supported forms:

```dosh
complete "cmd" [
  "value1"
  "value2"
]

complete "cmd" arg 1 {
  provider_fn
}

complete "cmd sub" flags {
  [
    "--raw"
    "--full"
  ]
}

complete "cmd" option "--target" {
  [
    "x86_64-pc-windows-msvc"
    "x86_64-unknown-linux-gnu"
  ]
}
```

## 4. Basic Static Completion

```dosh
complete "theme use" [
  "classic"
  "minimal"
  "powerline"
]
```

## 5. Completion Metadata (Rich Menu)

You can return records:

```dosh
complete "plugin" [
  { value: "install", description: "Install plugin", kind: "subcommand", icon: "📦", priority: 100 }
  { value: "remove", description: "Remove plugin", kind: "subcommand", icon: "🗑", priority: 90 }
  { value: "list", description: "List plugins", kind: "subcommand", icon: "📋", priority: 80 }
]
```

Supported fields:

- `value` (required)
- `description`
- `kind`
- `icon`
- `insert` or `insert_text`
- `priority`

## 6. `$ctx` Object

Dynamic providers can use `$ctx`:

- `line`
- `cursor`
- `words`
- `command`
- `args`
- `current`
- `previous`
- `position`
- `cwd`
- `is_flag`
- `flag`
- `command_path`

Example:

```dosh
complete "search-car" arg 2 {
  models $ctx.args.0
}
```

## 7. Full Example: Command + Completion in One File

Path: `~/.config/dosh/commands/search-car.dosh`

```dosh
mod search_car {
  export fn run($brand, $model) {
    print $"Searching ($brand) ($model)..."
  }

  export fn brands() {
    ["Toyota", "Honda", "BMW"]
  }

  export fn models($ctx) {
    match $ctx.args.0 {
      "Toyota" => ["Camry", "Corolla", "Fortuner"]
      "Honda" => ["Civic", "City", "CR-V"]
      "BMW" => ["X3", "X5"]
      _ => []
    }
  }

  complete "search-car" arg 1 {
    brands
  }

  complete "search-car" arg 2 {
    models $ctx
  }
}
```

Then reload:

```dosh
completion reload
```

Try:

- `search-car <TAB>`
- `search-car Toyota <TAB>`

## 8. Full Example: Advanced `devflow`

Path: `~/.config/dosh/commands/devflow.dosh`

```dosh
mod devflow {
  export fn run($action, $target) {
    print $"[devflow] action=($action) target=($target)"
  }

  complete "devflow" arg 1 cache 10sec timeout 500ms priority 100 {
    [
      { value: "build", description: "Build workspace", kind: "task", icon: "⚙", priority: 90 }
      { value: "test", description: "Run test suite", kind: "task", icon: "🧪", priority: 90 }
      { value: "lint", description: "Run lints", kind: "task", icon: "🔍", priority: 80 }
      { value: "run", description: "Run app", kind: "task", icon: "🚀", priority: 70 }
    ]
  }

  complete "devflow" arg 2 cache 5sec timeout 800ms {
    match $ctx.args.0 {
      "build" => {
        ls
        | where type == "dir"
        | where name != "target"
        | select name
        | get name
      }
      "test" => {
        glob "crates/*"
        | where type == "dir"
        | select name
        | get name
      }
      "lint" => {
        ["workspace", "crates", "cli", "core"]
      }
      "run" => {
        open Cargo.toml
        | get workspace.members
      }
      _ => []
    }
  }

  complete "devflow" flags {
    [
      { value: "--release", description: "Release mode", kind: "flag", icon: "🏁" }
      { value: "--watch", description: "Watch changes", kind: "flag", icon: "👀" }
      { value: "--target", description: "Target triple", kind: "flag", icon: "🎯" }
    ]
  }

  complete "devflow" option "--target" cache 60sec timeout 300ms {
    [
      "x86_64-pc-windows-msvc"
      "x86_64-unknown-linux-gnu"
      "aarch64-apple-darwin"
    ]
  }
}
```

## 9. Provider Options

Rule modifiers:

- `cache <duration>`: cache provider result, e.g. `cache 5sec`
- `timeout <duration>`: hard timeout, e.g. `timeout 300ms`
- `priority <number>`: higher first
- `no-filter`: disable auto filtering by current token

Example:

```dosh
complete "git checkout" arg 1 cache 5sec timeout 1sec priority 100 {
  branches
}
```

## 10. Completion-Only Scripts

Put in `~/.config/dosh/completions/git.dosh`:

```dosh
mod git_completions {
  export fn branches() {
    ^git branch --format="%(refname:short)"
    | lines
    | trim
  }

  complete "git checkout" arg 1 {
    branches
  }

  complete "git switch" arg 1 {
    branches
  }
}
```

## 11. Multiline Here-String for Fast File Creation

Dosh supports here-string:

```dosh
@'
mod sample {
  complete "sample" [
    "a"
    "b"
  ]
}
'@ | save "C:/Users/<you>/.config/dosh/commands/sample.dosh"
```

## 12. Debug Commands

Useful commands:

```dosh
completion list
completion show devflow
completion doctor
completion reload
```

Quick check:

```dosh
:complete devflow t
```

## 13. Common Issues

### `NO RECORDS FOUND`

Possible causes:

- Wrong command pattern in `complete "..."`.
- File not loaded yet (run `completion reload`).
- Current token filter removes all results.
- You are not in expected arg position.

### `program not found`

- You defined completion but not command `run`.
- Command file name does not match expected command name.

### Path errors on `save`

Use interpolated string:

```dosh
save $"($HOME)/.config/dosh/commands/x.dosh"
```

or absolute path.

## 14. Best Practices

- Keep command logic and completion logic in same file for small commands.
- Use `modules/` for shared providers.
- Add `cache` for expensive providers.
- Set `timeout` for external-heavy providers.
- Return record items for good UX (`description/kind/icon/priority`).
- Prefer structured pipeline outputs over plain text parsing where possible.

## 15. Suggested Workflow

1. Create command script in `commands/`.
2. Add `complete` rules.
3. Run `completion reload`.
4. Validate with `completion show <cmd>` and `:complete ...`.
5. Refine metadata and timeout/cache.

