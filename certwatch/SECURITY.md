# Security Policy

## Supported versions

CertWatch is at version `0.1.0-dev`. Only the latest commit on the default branch and the latest pre-release receive fixes.

## Reporting channel

Report vulnerabilities privately through the GitHub private vulnerability advisory form of the repository:

https://github.com/NexzaDev/workspace/security/advisories/new

The same link is in the application: About, Report a vulnerability privately. Do not open a public issue for a vulnerability.

## What to include

- The version (`certwatch version`) and how you installed it
- Operating system and architecture
- Steps to reproduce, with the smallest input that triggers the problem
- The impact you observed and the impact you expect is possible
- The output of `certwatch diagnostics`, with anything private removed

## Response expectations

This is a volunteer-maintained project without a guaranteed response time. Reports are read, acknowledged in the advisory thread and prioritised by impact. Fixes are released as a new pre-release.

## Scope

In scope: the application, its scripts, its build and release workflows, and the files it reads and writes.

Out of scope: vulnerabilities in third-party dependencies that are not reachable through the application (report those upstream), and findings that require an already compromised local user account.

## Disclosure

Please allow a fix to be released before disclosing details. The advisory is published with credit to the reporter unless the reporter asks otherwise.

The threat model is in [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).
