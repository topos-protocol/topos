Contributing
============

This repository operates an open contributor model where anyone is
welcome to contribute towards development in the form of peer review, testing
and patches. This document explains the practical process and guidelines for
contributing.

Contributor Workflow
--------------------

The codebase is maintained using the "contributor workflow" where everyone
without exception contributes patch proposals using "pull requests" (PRs). This
facilitates social contribution, easy testing and peer review.

To contribute a patch, the workflow is as follows:

  1. Clone the repository
  2. Create topic branch
  3. Commit patches

### Committing Patches

In general, [commits should be atomic](https://en.wikipedia.org/wiki/Atomic_commit#Atomic_commit_convention)
and diffs should be easy to read. For this reason, do not mix any formatting
fixes or code moves with actual code changes.

Make sure each individual commit is hygienic: that it builds successfully on its
own without warnings, errors, regressions, or test failures.

Commit messages should be verbose by default consisting of a short subject line
(50 chars max), a blank line and detailed explanatory text as separate
paragraph(s), unless the title alone is self-explanatory (like "Correct typo
in bar/foo.rs") in which case a single title line is sufficient. Commit messages should be
helpful to people reading your code in the future, so explain the reasoning for
your decisions. Further explanation [here](https://chris.beams.io/posts/git-commit/).

If a particular commit references another issue, please add the reference. For
example: `refs #1234` or `fixes #4321`. Using the `fixes` or `closes` keywords
will cause the corresponding issue to be closed when the pull request is merged.

Commit messages should never contain any `@` mentions (usernames prefixed with "@").

Please refer to the [Git manual](https://git-scm.com/doc) for more information
about Git.

  - Push changes to your branch
  - Create pull request

### Creating the Pull Request

The title of the pull request should be prefixed by the component or area that
the pull request affects. Valid areas as:

  - `protocol` for changes to protocol critical code
  - `doc` for changes to the documentation
  - `log` for changes to log messages
  - `net` or `p2p` for changes to the peer-to-peer network code
  - `refactor` for structural changes that do not change behavior
  - `rpc` or `rest` for changes to the RPC or REST APIs
  - `script` for changes to the scripts and tools
  - `test`, `qa` or `ci` for changes to the unit tests, QA tests or CI code
  - `config` for changes to cargo, toml, crates organization

Examples:

    protocol: Fetch the samples from the double echo
    net: Adapt the peer discovery with kademlia
    log: Fix typo in log message

The body of the pull request should contain sufficient description of *what* the
patch does, and even more importantly, *why*, with justification and reasoning.

The description for a new pull request should not contain any `@` mentions. The
PR description will be included in the commit message when the PR is merged and
any users mentioned in the description will be annoyingly notified each time a
fork copies the merge. Instead, make any username mentions in a
subsequent comment to the PR.

### Address Feedback

At this stage, one should expect comments and review from other contributors. You
can add more commits to your pull request by committing them locally and pushing
to your branch.

You are expected to reply to any review comments before your pull request is
merged. You may update the code or reject the feedback if you do not agree with
it, but you should express so in a reply. If there is outstanding feedback and
you are not actively working on it, your pull request may be closed.

Please refer to the [peer review](#peer-review) section below for more details.

### Squashing Commits

If your pull request contains fixup commits (commits that change the same line of code repeatedly) or too fine-grained
commits, you may be asked to [squash](https://git-scm.com/docs/git-rebase#_interactive_mode) your commits
before it will be reviewed. The basic squashing workflow is shown below.

    git checkout your_branch_name
    git rebase -i HEAD~n
    # n is normally the number of commits in the pull request.
    # Set commits (except the one in the first line) from 'pick' to 'squash', save and quit.
    # On the next screen, edit/refine commit messages.
    # Save and quit.
    git push -f # (force push to GitHub)

Please update the resulting commit message, if needed. It should read as a
coherent message. In most cases, this means not just listing the interim
commits.

Please refrain from creating several pull requests for the same change.
Use the pull request that is already open (or was created earlier) to amend
changes. This preserves the discussion and review that happened earlier for
the respective change set.

The length of time required for peer review is unpredictable and will vary from
pull request to pull request.

### Rebasing Changes

When a pull request conflicts with the target branch, you may be asked to rebase it on top of the current target branch.

    git fetch https://github.com/toposware/tce  # Fetch the latest upstream commit
    git rebase FETCH_HEAD  # Rebuild commits on top of the new base

This project aims to have a clean git history, where code changes are only made in non-merge commits. This simplifies
auditability because merge commits can be assumed to not contain arbitrary code changes.

After a rebase, reviewers are encouraged to sign off on the force push.

Pull Request Philosophy
-----------------------

Patch sets should always be focused. For example, a pull request could add a
feature, fix a bug, or refactor code; but not a mixture. Please also avoid super
pull requests which attempt to do too much, are overly large, or overly complex
as this makes review difficult.

### Features

When adding a new feature, thought must be given to the long term technical debt
and maintenance that feature may require after inclusion. Before proposing a new
feature that will require maintenance, please consider if you are willing to
maintain it (including bug fixing). If features get orphaned with no maintainer
in the future, they may be removed by the Repository Maintainer.

### Refactoring

Refactoring is a necessary part of any software project's evolution. The
following guidelines cover refactoring pull requests for the project.

There are three categories of refactoring: code-only moves, code style fixes, and
code refactoring. In general, refactoring pull requests should not mix these
three kinds of activities in order to make refactoring pull requests easy to
review and uncontroversial. In all cases, refactoring PRs must not change the
behavior of code within the pull request (bugs must be preserved as is).

### Peer Review

Code review is an important part of the development process, and as such, certain types of pull requests are rejected.

**As a reviewer proposing changes, you must request for those changes by choosing "[Request for changes](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/approving-a-pull-request-with-required-reviews)" when submitting your review.**
**In particular, you must not Approve the PR before acknowledgement of the changes that you requested.**

#### Conceptual Review

A review can be a conceptual review, where the reviewer leaves a comment
 * `Concept (N)ACK`, meaning "I do (not) agree with the general goal of this pull
   request",
 * `Approach (N)ACK`, meaning `Concept ACK`, but "I do (not) agree with the
   approach of this change".

A `NACK` needs to include a rationale why the change is not worthwhile.
NACKs without accompanying reasoning may be disregarded.

#### Code Review

After conceptual agreement on the change, code review can be provided. A review
begins with `ACK BRANCH_COMMIT`, where `BRANCH_COMMIT` is the top of the PR
branch, followed by a description of how the reviewer did the review. The
following language is used within pull request comments:

  - "I have tested the code", involving change-specific manual testing in
    addition to running the unit, functional, or fuzz tests, and in case it is
    not obvious how the manual testing was done, it should be described;
  - "I have not tested the code, but I have reviewed it and it looks
    OK, I agree it can be merged";
  - A "nit" refers to a trivial, often non-blocking issue.

Copyright
---------

By contributing to this repository, you agree to license your work under the MIT license. Any work contributed where you are not the original author must contain its license header with the original author(s) and source.

Changes to this arrangement
---------------------------

This is an experiment and feedback is welcome! This document may also be subject to pull-requests or changes by contributors where you believe you have something valuable to add or change.

Heritage
--------

Adapted from the contributing guidelines for the [bitcoin core](https://github.com/bitcoin/bitcoin/blob/master/CONTRIBUTING.md) repository.
