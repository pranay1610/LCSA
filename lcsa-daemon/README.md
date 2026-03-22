# lcsa-daemon

Filesystem event adapter for LCSA. Converts raw filesystem events into semantic JSONL signals.

## Installation

```bash
cargo install lcsa-daemon
```

## Usage

### Scan a directory

```bash
lcsa-daemon scan /path/to/project
```

### Watch a directory

```bash
lcsa-daemon watch /path/to/project --initial-scan
```

## Output Format

Each line is a JSON object representing a semantic signal:

```json
{
  "version": "0.1",
  "occurred_at": "2026-03-22T10:12:05Z",
  "source": "filesystem",
  "action": "updated",
  "entity_kind": "code",
  "summary": "Code file updated: src/main.rs",
  "confidence": 0.98,
  "paths": ["src/main.rs"],
  "tags": ["ext:rs", "topdir:src"],
  "metadata": {
    "primary_path": "src/main.rs",
    "extension": "rs"
  }
}
```

## Entity Kinds

| Kind | Extensions |
|------|------------|
| `code` | rs, py, js, ts, go, c, cpp, java, etc. |
| `config` | toml, yaml, json, ini, env, etc. |
| `docs` | md, txt, rst, adoc, etc. |
| `data` | csv, sqlite, parquet, etc. |
| `media` | png, jpg, mp4, etc. |

## License

Apache-2.0
