# Proposal: Semantic Search & Vector Indexing (pkg/search)

## Why
PRD v0.1.1 Task 5.2 requires semantic search capabilities to help users and Agents find the right API based on intent rather than exact path matches. This requires a local indexing mechanism that updates whenever the OpenAPI Spec is refreshed.

## What Changes
- Implement `pkg/search` package for indexing and searching.
- Implement a lightweight Vector/Keyword search engine (TF-IDF based for zero-dependency portability).
- Integrate indexing into `internal/auth/client.go` (trigger after cache update).
- Add `cjtc api search <query>` command.
- Persist the index to `~/.cjtc/index/<profile>.idx`.

## Impact
- **Specs**: Adds requirements for semantic discovery.
- **Code**: New `pkg/search` package.
- **Users**: Enables natural language discovery of APIs.
- **Performance**: Near-instant local search results.
