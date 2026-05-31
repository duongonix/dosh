$path = "C:\Users\duong\.config\dosh\commands\devflow.dosh"
$dir = Split-Path -Parent $path
if (-not (Test-Path -LiteralPath $dir)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

$content = @'
mod devflow {
  export fn run($action, $target) {
    print $"[devflow] action=($action) target=($target)"
  }

  export fn targets($ctx) {
    match $ctx.args.0 {
      "build" => {
        ls
        | where type == "dir"
        | select name
        | get name
      }
      "test" => ["crates", "tests"]
      "lint" => ["workspace", "crates", "cli", "core"]
      "run" => ["crates/dosh-cli", "crates/dosh-core"]
      _ => []
    }
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
    targets $ctx
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
'@

Set-Content -LiteralPath $path -Value $content -Encoding UTF8
Write-Host "wrote $path"
