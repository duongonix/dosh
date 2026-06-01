# Dosh Plugin Guide

This guide explains how to build, install, trust, and operate plugins in Dosh.

## 1. Plugin Model

Dosh plugins are extension packages that register commands into shell runtime.

Current direction:

- manifest-driven
- permission-aware
- trust/signature-friendly
- WASM runtime compatible

## 2. Plugin Lifecycle

Typical flow:

1. scaffold plugin
2. implement command logic
3. build plugin artifact
4. install plugin
5. enable plugin
6. run command
7. update / disable / remove

## 3. Create Plugin Scaffold

```dosh
dosh plugin init --name hello-http
```

This creates plugin structure with manifest template.

## 4. Build Plugin

If plugin targets wasm:

```dosh
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1
```

If target is missing, install target first.

## 5. Install Plugin

From local path:

```dosh
dosh plugin install --from ./hello-http
```

List:

```dosh
dosh plugin list
```

Enable:

```dosh
dosh plugin enable hello-http
```

Disable:

```dosh
dosh plugin disable hello-http
```

Remove:

```dosh
dosh plugin remove hello-http
```

## 6. Plugin Paths

Dosh uses shared plugin directory:

- Windows: `C:\Users\<you>\.config\dosh\plugins`
- Linux/macOS: `~/.config/dosh/plugins`

Use:

```dosh
config path
```

to inspect active directories.

## 7. Command Naming

Prefer namespace style command names:

- `hello-http.get`
- `hello-http.post`

This avoids collision with builtin commands.

## 8. Running Plugin Commands

After enable:

```dosh
hello-http.get https://httpbin.org/get
{name:"dosh"} | hello-http.post https://httpbin.org/post
```

If command not found:

- check plugin is enabled
- restart shell
- run `dosh plugin list`

## 9. Trust and Signature

Add trusted key:

```dosh
dosh plugin trust add --id org-key --public-key <base64>
```

List trusted keys:

```dosh
dosh plugin trust list
```

Remove trusted key:

```dosh
dosh plugin trust remove --id org-key
```

Sign plugin:

```dosh
dosh plugin sign --dir ./hello-http --key-id org-key --private-key <base64>
```

Verify signature:

```dosh
dosh plugin verify --dir ./hello-http --public-key <base64>
```

## 10. Plugin Permissions

Plugins should declare required capabilities (filesystem/network/process/env/secret).

Best practice:

- ask minimum permission
- avoid broad permission by default
- document why each permission is needed

## 11. Debugging Common Plugin Errors

### `empty wasm module`

Cause:

- wrong build output
- wasm file missing/empty

Fix:

- rebuild plugin target
- reinstall plugin from fresh build

### `failed to find function export alloc`

Cause:

- plugin ABI/export mismatch

Fix:

- use correct plugin template/runtime ABI
- rebuild with expected exports

### `unknown import wasi_snapshot_preview1::...`

Cause:

- target/runtime mismatch (`wasip1` vs non-wasi)

Fix:

- rebuild using runtime-compatible target

## 12. Publish Plugin

Local registry publish foundation:

```dosh
dosh plugin publish --from ./hello-http --registry ./registry
```

This prepares plugin artifact + metadata for registry flow.

## 13. Plugin Best Practices

- Keep plugin commands focused.
- Use structured JSON/record I/O in command contracts.
- Return clean errors (no panic).
- Version plugin with semver.
- Add README and examples for each command.

## 14. Suggested Plugin Testing

Before publish:

```dosh
dosh plugin install --from ./my-plugin
dosh plugin enable my-plugin
```

Then run:

- happy path command
- invalid args path
- network/filesystem denial path
- permission boundaries

## 15. Real-world Example Workflow

```dosh
dosh plugin init --name hello-http
# implement code
cargo build --release --target wasm32-wasip1
dosh plugin install --from ./hello-http
dosh plugin enable hello-http
hello-http.get https://httpbin.org/get
```

## 16. Security Guidance

- only install plugins from trusted source
- verify signature before enabling
- keep trust store minimal
- disable/remove unused plugins

