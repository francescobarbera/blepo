# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Philosophy

Code like Kent Beck. Prioritize simplicity, readability, and testability. Make the code clear enough that it doesn't need comments to explain what it does.

### Working Style

- **Proceed step by step.** Keep code changes small and incremental so each implementation detail can be reviewed before moving on.
- **Understand before changing.** Read existing code before modifying it. Understand the context, patterns, and constraints already in place.
- **Minimal changes.** Make the smallest change that solves the problem. Avoid refactoring unrelated code, adding unnecessary abstractions, or "improving" things that weren't asked for.

## Test-First Development

- **Domain logic**: Write the test before the implementation. Confirm it fails (`cargo test`), then write the minimal code to make it pass.
- **Infrastructure**: Use fixture-driven development (input fixture → expected output → implement).
- **Never add a function without a corresponding test in the same PR.**
- **Run `cargo test` after every change** — never leave tests in a broken state.

## Fixture-Driven Development

This project uses **Fixture-Driven Development** (FDD) - a test-first workflow optimized for AI-assisted coding where fixtures serve as the specification.

### The Workflow

1. **Human provides input fixture**: A sample input (e.g., RSS feed XML, channel config)
2. **Human provides expected output fixture**: The exact expected result (e.g., parsed video list)
3. **AI implements the transformation**: Code that converts input → output
4. **Test verifies**: Automated test compares actual output against expected fixture

### The Golden Rule: Fixtures Are Immutable

**NEVER modify input or expected output fixtures during implementation.**

Fixtures are the contract. They define what "correct" means. The implementation must conform to the fixtures, not the other way around.

```
✗ WRONG: "The expected output seems incorrect, let me fix it"
✗ WRONG: "I'll adjust the input to make parsing easier"
✓ RIGHT: "The test fails - I need to fix my implementation"
```

### When Fixtures Can Change

Fixtures should only change through **explicit human decision**, not as a side effect of implementation:

- **Requirements changed**: Human updates fixture to reflect new requirements
- **Fixture was wrong**: Human reviews and corrects a mistake in the fixture
- **New test case**: Human adds a new fixture for a new scenario

### Avoiding Overfitting

A risk with fixture-driven development is that AI might create code that only works for the specific fixtures rather than generalizing. To mitigate:

- **Multiple fixtures**: Provide several input/output pairs covering different cases
- **Edge cases**: Include fixtures for boundary conditions
- **Review the implementation**: Ensure logic is general, not hardcoded to specific fixture values

### Fixture Organization

```
tests/
├── fixtures/
│   ├── input/           # Input files (RSS XML, config files) (IMMUTABLE)
│   └── expected/        # Expected output (IMMUTABLE)
└── e2e/
    └── scenario_name/
        ├── input/
        └── expected/
```

Each scenario is self-contained with its own input and expected output.

## Documentation Maintenance

**Always update SPECIFICATIONS.md** when making changes to the codebase:
- Read `SPECIFICATIONS.md` before starting work to understand the current state
- Update `SPECIFICATIONS.md` after implementing new features or modifying existing behavior
- Keep the specifications in sync with the actual implementation

## Testing Requirements

**Always write tests.** When implementing a new feature or fixing a bug:
- Add unit tests for new functions and logic
- Add integration/e2e tests for new user-facing behavior
- Update existing tests when changing behavior
- Run `cargo test` after every change
- Ensure all tests pass before considering work complete

## Project Overview

**Blepo** is a Rust CLI tool for watching YouTube subscriptions without ads, distractions, or tracking. It fetches videos via RSS feeds and plays them through mpv + yt-dlp.

### Product Decisions

- **Playback**: mpv + yt-dlp (ad-free, private, no browser needed)
- **Video source**: YouTube RSS feeds (no API key, no auth, no tracking)
- **Fetch strategy**: on-demand only, fixed 7-day window
- **Video list**: flat chronological list, newest first, numbered entries
- **Channel management**: manual TOML config file
- **Shorts filtering**: YouTube Shorts detected via HTTP HEAD to `/shorts/<id>` (200 = Short, redirect/error = keep), filtered before display
- **Watched tracking**: marked as watched on play, no undo
- **Storage**: Platform-native paths via `directories` crate (macOS: `~/Library/Application Support/blepo/`, Linux: `~/.config/` and `~/.local/share/`). Only `watched.json` is persisted; videos are held in memory per session.
- **UX philosophy**: single command, fetch + list + prompt + play. No subcommands, no loops, no TUI.

### Core Workflow

```bash
blepo    # Fetch videos, show list, pick one to play
```

### Runtime Dependencies

- `mpv` — video player
- `yt-dlp` — YouTube stream extraction

### Domain Entities

- **Channel**: A YouTube channel the user subscribes to, identified by channel ID
- **Video**: A video from a channel, with title, URL, published date
- **WatchedVideo**: A video ID that has been played

### Key Dependencies

- `reqwest` — HTTP client for fetching RSS feeds
- `serde` + `quick-xml` — RSS/XML parsing
- `chrono` — date handling for the 7-day window
- `directories` — XDG path resolution

## Architecture

This project follows **Clean Architecture** (Hexagonal/Ports and Adapters):

```
src/
├── domain/           # Core business logic (entities, value objects)
├── application/      # Use cases and application services (ports/traits)
├── infrastructure/   # External concerns (RSS fetching, storage, mpv launching)
└── presentation/     # CLI entry point
```

### Layer Rules

1. **Domain layer** (innermost)
   - Business entities: `Channel`, `Video`, `WatchedVideo`
   - Value objects: `ChannelId`, `VideoId`
   - Pure business logic: filtering unwatched videos, sorting by date
   - NO dependencies on other layers or external crates (except std)

2. **Application layer**
   - Defines ports (traits): `trait FeedFetcher`, `trait VideoStore`, `trait VideoPlayer`
   - Contains use cases that orchestrate the workflow
   - Depends only on domain layer

3. **Infrastructure layer**
   - Implements application traits: RSS fetcher, JSON/SQLite store, mpv launcher
   - Depends on application layer (for trait definitions)

4. **Presentation layer**
   - Single interactive command with stdin prompt
   - Wires infrastructure implementations to application use cases
   - Error reporting to user

### Dependency Direction

```
presentation  → application → domain
infrastructure → application
```

- Domain has no dependencies (pure)
- Application depends on domain
- Infrastructure implements application traits
- Presentation wires everything together

## Code Style

### Parse, Don't Validate

Follow the **"Parse, Don't Validate"** principle: encode invariants into the type system so invalid states are unrepresentable at compile time.

- **Parse at the boundary, then trust the types.** Convert raw input (strings, bytes, untyped JSON) into domain types with validated invariants as early as possible. Once parsed, never re-validate — the type guarantees correctness.
- **Newtypes with fallible constructors.** Use the newtype pattern with private fields. Expose only constructors that enforce invariants (returning `Option` or `Result`), so invalid values cannot be created.
- **Prefer semantic types over primitives.** `ChannelId` not `String`, `VideoId` not `String`, `NonZeroF32` not `f32`. Types are documentation — seeing the type should communicate the invariant without reading code.
- **Never validate and discard.** If a function checks a property, it should return a type that preserves that knowledge. A `fn is_valid(x: &T) -> bool` is a code smell — prefer `fn parse(x: T) -> Result<ValidT, Error>`.

```
BAD:  fn verify(data: &Thing) -> bool     // caller must remember to check
GOOD: fn parse(raw: RawThing) -> Result<Thing, Error>  // invalid Thing cannot exist
```

### General Principles

- Functions do one thing
- Names reveal intent
- Make invalid states unrepresentable through the type system
- Prefer composition over inheritance; use traits for polymorphism

### Naming Conventions

- Use `snake_case` for file names: `expected_basic.json`, not `expected-basic.json`
- Separate words with underscores (`_`), never hyphens (`-`)

### Rust Conventions

- Use `Result<T, E>` for fallible operations
- Use the newtype pattern for domain types (`ChannelId`, `VideoId`, etc.)
- Use `#[must_use]` where ignoring return value is likely a bug

### Discriminated Unions

Prefer **discriminated unions** (Rust enums with associated data) over struct hierarchies or optional fields when modeling variants. This makes the type system enforce valid states.

### SOLID Principles

**S - Single Responsibility**: Each module/struct has one reason to change.
- `RssFeedFetcher` only fetches and parses RSS feeds
- `JsonVideoStore` only persists watched state
- `MpvPlayer` only launches mpv with yt-dlp

**O - Open/Closed**: Open for extension, closed for modification. Use traits to allow new implementations without changing existing code.

**L - Liskov Substitution**: Implementations must be substitutable for their trait.

**I - Interface Segregation**: Prefer small, focused traits over large ones:
- `FeedFetcher` — fetching only
- `VideoStore` — persistence only
- `VideoPlayer` — playback only

**D - Dependency Inversion**: High-level modules depend on abstractions, not concrete implementations.

## Error Handling

Use custom error types without external crates:

- Define specific error enums per layer (e.g., `FetchError`, `StoreError`, `PlayError`)
- Implement `std::error::Error` and `Display` manually
- Propagate errors with `?` operator
- Convert between error types at layer boundaries

## Testing Strategy

### End to End Tests

- Use fixture RSS XML as input
- Verify the full pipeline: fetch → filter → list
- Test watched state persistence across commands

### Unit Tests

- Domain logic: test filtering, sorting, date windowing
- RSS parsing: test XML extraction against known feed formats
- Keep unit tests in the same file as the code (`#[cfg(test)]` module)

## Logging

Use simple `println!` or `eprintln!` for info-level logging. Keep it minimal:
- Log when fetching a channel
- Log summary after fetch (channels fetched, new videos found)
- Log errors to stderr

No external logging crates for now.

## Build & Test Commands

```bash
cargo build              # Development build
cargo build --release    # Release build
cargo test               # Run all tests
cargo test <test_name>   # Run a specific test
cargo clippy             # Run linter
cargo fmt                # Format code
cargo fmt -- --check     # Check formatting without changes
```

## CI/CD

GitHub Actions runs on every push and PR:

```yaml
# .github/workflows/ci.yml
- cargo fmt --check
- cargo clippy -- -D warnings
- cargo test
```

## CLI Usage

```bash
blepo    # Fetch videos, show list, pick one to play
```

### Config File

Platform-dependent path resolved by the `directories` crate. Run `blepo` to see the expected path. Typical locations:
- **macOS**: `~/Library/Application Support/blepo/config.toml`
- **Linux**: `~/.config/blepo/config.toml`

```toml
# Number of days to look back when fetching
fetch_window_days = 7

[[channels]]
name = "Channel Name"
id = "UCxxxxxxxxxxxxxxxxxxxxxx"
```

## Post-Implementation Refactoring

After completing each implementation, always refactor the code:
- Remove dead code
- Simplify what can be simplified (redundant branches, nested conditions, duplicate logic)

## What NOT to Do

- Don't add features beyond what was asked
- Don't refactor unrelated code
- Don't add unnecessary comments, docstrings, or type annotations to unchanged code
- Don't add error handling for scenarios that can't happen
- Don't create abstractions for one-time operations
- Don't design for hypothetical future requirements
