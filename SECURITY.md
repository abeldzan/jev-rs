# Security Policy

## Supported versions

Security fixes are provided for the latest released version. Users should
upgrade to the newest patch release before reporting a vulnerability that may
already have been addressed.

## Reporting a vulnerability

Do not report security vulnerabilities in public issues, discussions, pull
requests, or chat channels. Use the repository's private GitHub security
advisory form instead:

1. Open the repository's **Security** tab.
2. Select **Advisories** and then **Report a vulnerability**.
3. Include affected versions, impact, reproduction steps, and any suggested
   remediation. Remove API keys, customer data, and other secrets.

Maintainers will acknowledge a report as soon as practical, investigate it,
and coordinate disclosure and a fix with the reporter. Please allow time for a
patch to be prepared before publishing details.

## Credential exposure

If a TypeSafe API key is exposed, revoke or rotate it immediately. Removing a
key from a later commit does not remove it from repository history.
