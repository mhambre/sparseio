When reviewing code, prioritize high-signal issues that a competent human reviewer would genuinely care about.

Focus review comments on:

* Correctness bugs, including edge cases, invalid assumptions, race conditions, error-handling problems, and behavior that can produce incorrect results.
* Security vulnerabilities or meaningful security regressions.
* Performance regressions or unnecessarily expensive behavior when the impact is material.
* Broken or incomplete functionality.
* Backwards-compatibility issues or unintended behavioral changes.
* Resource leaks, lifecycle problems, concurrency issues, or reliability concerns.
* Missing validation or defensive handling where realistic inputs can cause failures.
* Missing or incorrect tests when the changed behavior is important enough that a regression would matter.
* Missing documentation only when the documentation is genuinely necessary for users, maintainers, public APIs, configuration, non-obvious behavior, or operational concerns.
* Violations of established project coding standards, architectural conventions, or patterns when the deviation is meaningful and should be corrected.

Do not be pedantic.

Avoid comments about:

* Purely subjective style preferences.
* Minor naming preferences when the existing name is understandable.
* Trivial formatting issues that should be handled by formatters or linters.
* Hypothetical edge cases with no realistic likelihood or meaningful impact.
* Additional comments, documentation, abstractions, helper methods, tests, or refactors that would provide little practical value.
* "Best practice" suggestions that do not solve an actual problem in the submitted change.
* Micro-optimizations without evidence of meaningful performance impact.
* Refactoring working code solely to make it marginally cleaner or more elegant.
* Suggestions that are already enforced by automated tooling.
* Restating what the code does without identifying a concrete problem.

Before leaving a comment, ask whether a reasonable human reviewer would consider the issue important enough to block, request changes, or explicitly call out during review. If not, omit it.

Prefer fewer, higher-confidence comments over exhaustive feedback. Do not invent potential problems merely to provide review feedback. If the change is correct, maintainable, performant, appropriately documented, and consistent with the project's standards, it is acceptable to leave no comments.

For every issue raised:

1. Identify the concrete problem.
2. Explain the realistic impact or failure mode.
3. Point to the relevant code.
4. Suggest a practical fix when one is reasonably clear.

Treat review findings roughly in this priority order:

1. Correctness and broken behavior
2. Security and data integrity
3. Reliability and concurrency
4. Material performance regressions
5. API or compatibility problems
6. Missing tests or documentation with real maintenance/user impact
7. Meaningful violations of project conventions

Optimize for signal, not comment count.

