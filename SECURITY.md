# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in MeraRAW, please report it privately to help us address it responsibly before public disclosure.

### How to Report

**Email**: [security@meratech.co](mailto:security@meratech.co)

Please include:

- A description of the vulnerability
- Steps to reproduce the issue
- Potential impact
- Any suggested fixes (if applicable)

### What to Expect

- **Acknowledgment**: We will acknowledge receipt of your report within 48 hours
- **Updates**: We will provide updates on the status of your report
- **Resolution**: We will work to address confirmed vulnerabilities in a timely manner
- **Credit**: With your permission, we will credit you in the security advisory

### Scope

Security vulnerabilities we are particularly interested in:

- Code execution vulnerabilities
- Memory safety issues in RAW decoding
- Path traversal or file system access issues
- Authentication bypass (for license verification)
- Data exfiltration vulnerabilities
- Privilege escalation issues

### Out of Scope

- Denial of service through malformed RAW files (expected behavior: per-file error, not crash)
- Issues requiring physical access to the user's machine
- Social engineering attacks

## Security Best Practices

MeraRAW implements several security measures:

- **RAW file parsing**: Uses pure-Rust `rawler` decoder to minimize memory safety risks
- **Sandboxing**: Tauri provides OS-level sandboxing
- **Network restrictions**: Minimal network access; external URLs require explicit user consent
- **File access**: Scoped file system permissions via Tauri's security model
- **License verification**: Offline JWT verification with public key
- **No telemetry by default**: Analytics are opt-in and use minimal data

## Responsible Disclosure

We follow responsible disclosure practices:

1. Security issues are fixed before public disclosure
2. Affected users are notified when updates are available
3. Security advisories are published after fixes are released
4. We credit researchers who report vulnerabilities (with their permission)

## Supported Versions

Security updates are provided for:

- The latest stable release
- The current development branch (main)

Older versions may not receive security updates. Please upgrade to the latest version.

## License Verification Security

MeraRAW uses ES256 JWT signatures for license verification. The public key is embedded in the application. Private keys are never distributed. If you discover issues with the license verification system, please report them through the security channel above.

## Update Policy

- **Critical vulnerabilities**: Patched and released as soon as possible
- **High severity**: Patched within 30 days
- **Medium/Low severity**: Included in the next regular release

Thank you for helping keep MeraRAW secure!
