# Contributing

## Development Setup

```bash
pnpm install
pnpm dev
```

## Quality Checks

Run all checks before creating a PR:

```bash
pnpm typecheck
pnpm build
pnpm test
pnpm test:e2e
```

## Commit Style

Use conventional commit prefixes:

- `feat:`
- `fix:`
- `docs:`
- `test:`
- `chore:`

## Pull Request Checklist

1. Scope is focused and minimal.
2. README/docs updated if behavior changed.
3. All checks pass locally.
4. New functionality includes tests (where applicable).
