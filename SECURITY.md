# Security Policy

## Supported Versions

OpenJax is evolving quickly. Security fixes are generally applied to the latest `main` branch first.

## Reporting a Vulnerability

Please do not open a public issue for security vulnerabilities.

Use one of these channels:

- GitHub Security Advisories (preferred): use the "Report a vulnerability" feature in this repository.
- Private contact: open an issue with minimal detail and request a private follow-up channel if Advisory submission is unavailable.

When reporting, include:

- Affected component/module
- Reproduction steps or proof of concept
- Impact and expected severity
- Suggested mitigation (if available)

## Response Expectations

- Initial triage target: within 3 business days
- Status update target: within 7 business days
- Fix timeline depends on severity and complexity

## Hardening Notes

OpenJax includes sandbox and approval controls, but they are not a guarantee against all hostile inputs.
Operators should:

- Run with least-privilege credentials
- Review tool approval policy before production use
- Isolate sensitive environments and secrets
