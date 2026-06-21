# dotdiff

[![CI](https://github.com/rvben/dotdiff/actions/workflows/ci.yml/badge.svg)](https://github.com/rvben/dotdiff/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/dotdiff.svg)](https://crates.io/crates/dotdiff)
[![clispec](https://img.shields.io/badge/clispec-v0.2-blue)](https://clispec.dev)

Semantic diff for JSON, YAML, TOML, and NDJSON. Compares two documents
*structurally* (not line by line) and prints a path-addressed change list
instead of textual noise. The sibling of [`dotpick`](https://github.com/rvben/dotpick):
dotpick extracts fields, dotdiff tells you what changed.

## Install

```sh
cargo install dotdiff
```

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
subcommand whose output validates against `clispec.dev/schema/v0.2.json`
(checked by the test suite), and the exit-1-on-differences contract declared as
an `outcome`. It is read-only (`mutating: false`).

```sh
dotdiff schema
```

## License

MIT
