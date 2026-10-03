# dotdiff

[![CI](https://github.com/rvben/dotdiff/actions/workflows/ci.yml/badge.svg)](https://github.com/rvben/dotdiff/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/dotdiff.svg)](https://crates.io/crates/dotdiff)
[![clispec compliant](https://img.shields.io/badge/clispec-compliant-3b82f6)](https://clispec.dev)

Semantic diff for JSON, YAML, TOML, and NDJSON. Compares two documents
*structurally* (not line by line) and prints a path-addressed change list
instead of textual noise. The sibling of [`dotpick`](https://github.com/rvben/dotpick):
dotpick extracts fields, dotdiff tells you what changed.

## Install

```sh
cargo install dotdiff
```

### Nix

With Nix’s `nix-command` and `flakes` features enabled, packages are available
for Linux x64/ARM64 and Apple Silicon macOS:

```sh
nix run github:rvben/dotdiff -- --help
nix build github:rvben/dotdiff
```

Both `Cargo.lock` and `flake.lock` are committed. The package runs the Rust test
suite and checks the installed command and Bash, Fish, and Zsh completions.
Intel Macs are not supported by the pinned nixpkgs; use the other installation
methods above.

For NixOS or Home Manager, add the input to your flake:

```nix
inputs.dotdiff = {
  url = "github:rvben/dotdiff";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Then add `inputs.dotdiff.packages.${pkgs.stdenv.hostPlatform.system}.default`
to `environment.systemPackages` (NixOS) or `home.packages` (Home Manager).
Pass `inputs` through `specialArgs` for `nixosSystem` or `extraSpecialArgs` for
`homeManagerConfiguration`. The overlay `inputs.dotdiff.overlays.default`
also provides `pkgs.dotdiff` using your package set and Rust toolchain overrides.
Following your own nixpkgs uses its toolchain; it must meet the Rust version
required by `Cargo.toml` and support your platform.

For development:

```sh
nix develop
nix flake check                  # build, Rust tests, installed-command checks
nix fmt -- --check flake.nix nix/*.nix
```

`direnv allow` is optional and requires nix-direnv. The development shell
includes the package's native build dependencies and Rust development tools.
Update Nix inputs deliberately with `nix flake update`, review the lockfile,
and run the checks before committing it.

## Usage

```sh
dotdiff old.json new.json          # text on a TTY, JSON when piped
dotdiff config.yaml config.json    # cross-format works
cat new.json | dotdiff old.json -  # `-` reads stdin
```

```text
$ dotdiff old.json new.yaml
~ user.plan       "free" -> "pro"
- user.trialEnds  "2026-07-01"
+ user.seats      5
~ items[0].qty    1 -> 3
```

Output is the `~`/`+`/`-` change list on a TTY, and structured JSON when piped:

```sh
$ dotdiff old.json new.json | jq .
{"identical": false, "changes": [
  {"op": "changed", "path": "user.plan", "old": "free", "new": "pro"}
]}
```

Paths use [dotpick](https://github.com/rvben/dotpick)'s dotpath vocabulary
(`user.plan`, `items[2].qty`, `["quoted key"]`), so you can feed a changed path
straight back into `dotpick` to inspect it.

### Matching list items by key

By default arrays compare by position, so reordering a list reports everything
after the move as changed. Pass `--array-key <field>` to match objects in a list
by an identity field instead - order-independent, and far less noise:

```sh
# Without: a reordered list looks like 4 changes.
# With --array-key id: just the one real change.
$ dotdiff old.json new.json --array-key id
~ items[id=1].qty  1 -> 3
```

### Formats

JSON, YAML, TOML, and NDJSON are detected per file (by extension, then content);
force both sides with `--format`. NDJSON is compared as an array of records, so
`--array-key` matches records across two streams. Everything is loaded into one
model, which is why cross-format diffing works.

## Exit codes

| code | meaning |
| --- | --- |
| `0` | identical |
| `1` | differences found (the report is on stdout - not an error) |
| `2` | an input could not be read or parsed |
| `3` | usage error |

Exit `1` is a *data state*, not a failure (the `diff`/`grep` convention), so
dotdiff is scriptable as a gate: `dotdiff a.json b.json && echo unchanged`.

## For agents (clispec)

dotdiff follows [The CLI Spec](https://clispec.dev): structured output on
stdout, structured error envelopes on the last line of stderr, a `schema`
subcommand whose development-branch output validates against the candidate
`clispec.dev/schema/v0.3.json` (checked by the test suite), and the
exit-1-on-differences contract declared as an `outcome`. Every command declares
`effects: read_only`; the published release remains on frozen v0.2 until v0.3
freezes.

```sh
dotdiff schema
```

## License

MIT

## Releasing

Vership owns versioning, changelog generation, release commits, and tags. See
[the release runbook](docs/releases.md) for the verified workflow and recovery policy.
