# 0002. Settings live in the database

Status: accepted

## Context

Settings must be visible and changeable in every interface and must follow the data when the database is moved or copied. The application also has to find the database before it can read anything from it.

## Decision

Every setting is a row in the `settings` table, defined with a kind, a default and validation in the `settings` module of `secretsentry-core`. The only values outside the database are the database path and the remembered interface choice, in `config.json` in the user configuration directory, created for the current user only.

Flags and environment variables may override a setting for one run. An override never changes the stored value and is always shown with its source.

## Consequences

- Copying the database file carries the settings with it.
- An unknown or invalid stored value is kept and reported, so moving between versions loses nothing.
- Window size is a setting like any other and can be reset.
