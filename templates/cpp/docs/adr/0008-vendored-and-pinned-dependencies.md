# 0008. SQLite vendored, other libraries pinned through vcpkg

Status: accepted

## Context

C++ has no standard package manager and no lock file. The build must be repeatable on three operating systems and three compilers, every component must be known with its license, and a vulnerability in a component must be found by a check rather than by chance.

## Decision

- SQLite is vendored as its amalgamation in `third_party/sqlite`. It is two files, it is in the public domain, and the application needs specific compile options (FTS5).
- The other libraries (FTXUI, JSON for Modern C++, Dear ImGui, GLFW, doctest) come from vcpkg in manifest mode. `vcpkg.json` names a baseline commit and an exact version override for each library, and `scripts/dev-setup.sh` installs vcpkg itself at the same commit. The dashboard libraries and the test library are manifest features, so the no-GUI build does not fetch them.
- The build does not depend on vcpkg being present: any location that provides the same CMake packages can be given in `CMAKE_PREFIX_PATH`.
- `third_party/components.tsv` lists every component, including code that a library bundles, with license, license text and upstream commit. The notices are generated from it. `make vuln` checks the list against `vcpkg.json` and asks the OSV database about each commit.
- Libraries are linked statically, and on Windows the C++ runtime too.

## Consequences

- The first build compiles the libraries from source and takes several minutes; vcpkg keeps the results for later builds, and CI caches them.
- Updating a library is one change in three places that the checks hold together: the override in `vcpkg.json`, the row in the component list (version and commit), and the regenerated notices.
- Code bundled inside a library is found by reading its source when the row is added. No tool lists it, so the review is a manual step, described in [../LICENSES.md](../LICENSES.md).
- The OSV lookup is by commit. A component without a recorded commit is not looked up; the check refuses a top-level component without one.
