<!-- SPDX-License-Identifier: MIT -->

# Getting help

Three destinations, and they are not interchangeable. Picking the right one is
the difference between an answer and a thread nobody is paged for.

## I have a question

Read first, because the answer is often already written and is more precise
than a reply:

- **[The documentation site](https://rubentalstra.github.io/notio/):** the
  landing page, and the [handbook](https://rubentalstra.github.io/notio/docs/)
  covering what Notio is, how to evaluate it, how to integrate a FHIR client,
  and how to operate the server.
- **[`docs/architecture.md`](docs/architecture.md):** the design, in one file,
  with the citations behind each decision.

If the documentation does not answer it, **open a GitHub issue** through the
[issue chooser](https://github.com/rubentalstra/notio/issues/new/choose): how
to configure something, whether an approach fits, what a FHIR terminology
operation or an ECL construct means in this implementation, or why a design is
the way it is.

There is no commercial support offering, no service-level agreement, and no
paid tier. Answers come when the maintainer is at a keyboard
([MAINTAINERS.md](MAINTAINERS.md) is honest about how many keyboards that is).

## I found a defect

**[Open an issue](https://github.com/rubentalstra/notio/issues/new/choose)**
when something is wrong, missing, or contradicts the FHIR or SNOMED CT
specifications.

The reports that get fixed fastest carry:

- the version (the release tag or commit), and how it is deployed, including
  the SNOMED edition loaded;
- the request and the response, verbatim (method, path, headers, bodies), or
  the ECL expression and the expansion it produced;
- what the specification says should have happened, with the FHIR operation
  page or the ECL grammar section if you have it. A citation turns a
  disagreement into a defect.

**A specification-conformance report is the most valuable kind here.** The
implementation is never presumed correct because it was written to the
specification; the specification text is the authority and the implementation
is the usual culprit. Snowstorm and Hermes are prior art, so "another server
does it differently" is not by itself a defect; a spec citation is.

## I found a vulnerability

**Do not open a public issue.** Follow [SECURITY.md](SECURITY.md): report
privately through
[GitHub private vulnerability reporting](https://github.com/rubentalstra/notio/security/advisories/new).

That document also carries what you can expect in return: an acknowledgement
window, an assessment window, and coordinated disclosure with credit by
default.

**A vulnerability in a dependency, or in a service you deployed alongside
Notio, goes to that project**, not here, unless it has a Notio-specific impact.
SECURITY.md § *Scope* has the routing.

## I want to change something

[CONTRIBUTING.md](CONTRIBUTING.md) is the practical guide: setup, the gates
every pull request must pass, and the hard rules. [GOVERNANCE.md](GOVERNANCE.md)
is how the decision gets made and how someone becomes a maintainer.

## What you are entitled to

Nothing, and that is worth saying plainly. Notio is MIT-licensed software
provided as-is, with no warranty. Read the [LICENSE](LICENSE), which says
exactly that in the language that binds. Everything above describes what the
project *intends* to do, and the intent is sincere; none of it is a
contractual commitment, and only the security-report windows in SECURITY.md
are stated as promises at all.

Only the newest release receives fixes ([SECURITY.md § Supported
versions](SECURITY.md#supported-versions)). If your deployment needs a stronger
guarantee than a single-maintainer project can give, the honest options are to
fork and maintain, or to fund the capacity that would change the answer.
