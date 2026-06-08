# Security Policy

## Reporting a Vulnerability

Morphic is pre-alpha research software. If you discover a security vulnerability:

1. **Do NOT open a public issue.**
2. Email the maintainer directly or use GitHub's private vulnerability reporting.
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Impact assessment
   - Suggested fix (if any)

## Scope

Security concerns for Morphic include:

| Area | Concern |
|---|---|
| **Code generation** | Generated code must not contain injection vulnerabilities |
| **LLM integration** | Prompt injection through crafted specs |
| **SMT solver** | Z3 solver crashes or resource exhaustion |
| **Parser** | DoS via malformed .morph files |
| **IR** | Infinite recursion in synthesis engine |

## Supported Versions

| Version | Supported |
|---|---|
| v0.1.x (pre-alpha) | ⚠ Best effort |

## Acknowledgments

We appreciate responsible disclosure. Contributors who report valid security issues will be acknowledged (with permission).
