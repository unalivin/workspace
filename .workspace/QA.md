# Questions and answers for the build agent

This file lives at `.workspace/QA.md` and is removed with the rest of `.workspace/` at the end. It answers the questions that come up while building. If your question is here, follow the answer. If it is not, choose the simplest option that fits the main prompt, write one line in `.workspace/DECISIONS.md` and continue. You never ask the maintainer anything.

## Process

**Do I ask the maintainer to confirm something before I continue?** No. Never. Decide, record, continue.

**The maintainer only wrote "continue". What do I do?** Run `scripts/measure.sh` on every project marked Code complete or Complete, demote any that fail, then take the first project that is not Complete and carry on from its "next step" line. Do not recap.

**Which project do I start with?** The first one in the build order that is not Complete or Code complete. If the repository already holds projects marked Complete, measure them first. A project that does not meet its floor is not Complete, whatever the status file says.

**Can I start the next project while the current one is below its floor?** No. Finish the current one. A thin project copied forward makes every later project thin.

**My budget is almost gone. What do I do?** Finish the current milestone so the tree builds, update `.workspace/STATUS.md` with the true state, push, and write at most ten lines in Greek: which projects are Complete or Code complete, which is in progress and at which step, what comes next. No questions, no options.

**May I mark a project Complete because the files exist?** No. Complete is what `scripts/measure.sh` and the CI runs say. Files existing proves nothing.

**What if a command I need is not installed and I cannot install it?** Work with what is installed, write the code carefully, let CI compile and test it, and read the result through check-run annotations. Keep the project In progress until the checks have really passed. Do not claim a run that did not happen.

**How many CI fix rounds may I use?** Eight per project. After that mark the failing area as partial in the project's `docs/KNOWN_ISSUES.md` in neutral words, keep everything else green and move on.

## What the maintainer wants

**Who will use these projects?** People who download a release or clone a repository of one project. They want a tool that works, is pleasant to use, and a repository that looks cared for.

**What happens to each project folder?** The maintainer downloads it and uploads it as a new repository. So a folder must stand alone: its README, docs, license, build files and assets all live inside it, and nothing inside points at the monorepo, at `.workspace/`, or at another project.

**Which files must a folder not contain?** Tests, fuzz targets, test data, bats files, TESTING and ROADMAP documents, ADRs, duplicated contributing documents, development scripts, process notes, anything that only helped build the project. You still write and run tests while building. `scripts/strip-dev-files.sh` moves them out when the project is finished. See "What a project folder contains" in the main prompt.

**Should the projects be small or large?** Large. Each one is a full product with every screen in its table, a real service layer, a CLI, a terminal UI, a dashboard, a rich Settings screen, notifications, a packaged release, good pictures and complete docs. The floors in `scripts/floors.tsv` are minimums, not goals. If you reach a floor and the brief still has unbuilt logic, keep building.

**What if the brief seems smaller than the floor?** It is not. The brief's "Real logic" list is the minimum depth; every screen can also have filters, saved views, bulk actions, exports, comparisons, keyboard shortcuts and detail panels. Add those before you consider lowering a floor.

**Do the tests count towards the line floors?** No. Floors count production code only. Tests have their own floor and are not shipped.

**Which language do I use?** Greek in every message to the maintainer, whatever language they write in. English for the repository: code, commits, documents, interface text.

**How do I think about the work?** Like the most capable engineer you can imagine: design the data model and failure modes first, question your first idea, find root causes, verify claims by running them, prefer complete and correct over fast. Do not narrate it; the quality shows in the work.

## Design and identity

**How do I choose colours?** Use the project's row in Appendix C of the main prompt: a brand colour and a highlight colour. They colour the icon and are the first-run theme (the Brand preset). Do not invent a different palette.

**How are the colours spread over a screen?** 60-30-10. About 60 percent is the base, a quiet neutral derived from the brand colour. About 30 percent is the brand colour itself (sidebar, top bar, headers, selected rows, primary buttons). About 10 percent is the highlight, one bright element per screen. The result should look like a serious, recognisable brand.

**What colours do charts use?** The data colour system in "Charts and data colour", the same in every project: eight categorical colours for unordered categories, fixed severity colours, a sequential ramp built from the brand colour, a diverging ramp, and the highlight only for the focused series. A category keeps the same colour in every chart, table marker and export. Every distinct item, such as a vendor and its product, gets its own colour, shown as a marker beside its name. Beyond eight items, colours are generated by hue stepping and stored so they never change.

**How polished must it be?** Like software a large company would adopt: consistent layout, dense and aligned tables, full accessibility, every number traceable to its records, every setting documented. See "Enterprise standard" in the main prompt. Never write a company name as an inspiration anywhere in the repository.

**How good must graphs, statistics and tables be?** Like the analytics screens of the best enterprise products: a framed chart with title, unit and time range, hover and zoom, threshold lines, key numbers with change against the previous period, tables with sticky headers, pinned and grouped columns, data bars, saved views and export. See "Craft of every chart, statistic and table" in the main prompt.

**What colour is the text?** Only near-black (`#0B0F14`) or white. Never grey text and never coloured text. Hierarchy comes from size and weight. Text on the base is ink in the light theme and paper in the dark theme, text on the brand colour is paper, text on the highlight is ink.

**What does "the icon and the theme are synced" mean?** The icon is drawn with the project's brand and highlight colours. On first run the application uses those colours as its theme. If the user changes the theme or the accent, the logo drawn inside the application is redrawn with the new colours. The operating system icon files stay in the brand colours. The "Reset to brand colours" action restores the first-run look.

**How good do the icon and the diagrams have to be?** Good enough that a designer would not be embarrassed. Follow "Image quality" in the main prompt. Render them, look at them at real sizes, redraw what is uneven, crowded or generic. Never describe a picture you did not render.

**Where do the diagrams get their colours?** From the project's `assets/palette.json`, one colour per layer.

**Which toolkit looks do I keep?** None. The default look of Fyne, egui, Dear ImGui and the terminal libraries must be replaced by the project's own theme tokens.

**Do I mention the design guidance I was told to read?** Never. Describe only the resulting decisions.

## Writing and repository rules

**Do I write comments in code?** No comments in any source file. Names carry the meaning.

**Where do email addresses go?** Nowhere in the repository. Contact is the private vulnerability reporting form and issues.

**How do I word README and docs?** Like a maintainer talking to a colleague. Concrete, plain, with the file names, commands and numbers. No marketing words, no emoji, no em dashes, no closing summaries. The README follows the structure in the main prompt.

**Do I mention how the work was produced?** Never. No words such as AI, assistant, agent, model, prompt, generated, sandbox, round. No trailers in commits.

**What do commits look like?** Conventional Commits with the project scope, small, specific, imperative, for example `feat(certwatch): add expiry thresholds`. No `wip`, no `fix ci`, no retry commits.

**Which license and which copyright holder?** MIT. "Sonix Devs" for the 17 company projects and "Unalivin" for the 8 personal ones, as in the registry. The root `LICENSE` names "Unalivin".

**Which owner goes in links and module paths?** `github.com/sonix-devs/<project>` for company projects and `github.com/unalivin/<project>` for personal ones, lowercase.

## Scope of the product

**Is there a website or a hosted part?** No. Each project is a local desktop application with a dashboard, a terminal UI and a CLI over one service layer. Optional listeners are off by default and bind to loopback only.

**Do I use paid services?** Never. Everything must work for EUR 0. External sources are optional adapters with an offline demo mode.

**Does any project need network access to work?** No. Network features are off by default, listed in Settings under Network with their exact destination, and each has its own switch.

**What does demo data look like?** Realistic, synthetic, consistent across screens, labeled as demo data wherever it appears. `<app> demo load` fills every screen so a fresh install shows something real. It is a feature of the product, not a test fixture, and it ships with the application.

**Can a scanner or probe touch anything it likes?** No. Active checks act only on explicitly configured, authorized scope, with limits, timeouts and a confirmation.

**May a feature exist in the interface without working behind it?** No. Every visible control performs a real action.

**What do I do about a dependency whose license is not MIT?** Use it only if its license is permissive and compatible, and list it in `THIRD_PARTY_NOTICES.md` with its real license. Never describe third-party code as MIT.

**Which UI libraries do I use?** Go: Fyne for the dashboard and Bubble Tea for the terminal. Rust: eframe and egui, ratatui and crossterm. C++: Dear ImGui with GLFW and OpenGL, and FTXUI. Do not switch libraries.

**Do I add a second native language for speed?** Only when a benchmark proves at least a 2x gain, as the main prompt says. Otherwise do not.

## Quality

**What is the order of work inside a project?** Migrations and SQL tests, service layer and its tests, CLI, terminal UI, dashboard, notifier, then icon, docs, diagrams, CI and release workflow, then the audit. Do not write documents before the features exist.

**How do I know a screen is good enough?** It has real data from the service layer, a table with sorting, filtering, columns, detail panel and export, the loading, empty, error and success states, keyboard operation, and one well-made visual where the brief asks for one. A label with a number is not a screen.

**How deep must the Settings screen be?** Every section in "The Settings screen" of the main prompt, plus every threshold, interval, limit and policy named in the project's brief. Each setting has a description, a default and an override indicator.

**What do I do with bugs I find and cannot fix?** Write them in the project's `docs/KNOWN_ISSUES.md` in neutral maintainer words: symptom, affected platforms, workaround.

**Where does the honest status of a project live?** In `.workspace/STATUS.md` and `.workspace/audit/<project>.md`, backed by the output of `scripts/measure.sh`. Never in the project folder.

## Release

**Do I create tags, releases or repositories?** Never. The maintainer does that. You prepare workflows, release notes and the three release files' build.

**Which three files does a release have?** `<project>-linux-x64.tar.gz`, `<project>-macos-arm64.tar.gz`, `<project>-windows-x64.exe`. No checksums file. Release notes explain how to compare the digest GitHub shows.

**Which branch do I use and may I merge?** The branch the environment provides. Push often, never force-push, never merge.

**What happens to `.workspace/` at the end?** After the last project is Complete, remove it in a final commit named `chore: remove working notes`. It holds this file, the status, the decisions, the audits and the archive of development files.
