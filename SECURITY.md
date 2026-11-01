# Security policy

Strada is pre-1.0. The latest release receives fixes.

Please report vulnerabilities privately through GitHub: *Security → Report a vulnerability* on https://github.com/6t9xstar/strada. Do not open a public issue. Expect an acknowledgement within 3 working days.

Built-in protections (all configurable through `Server` / `Limits`): request body limit (2 MiB), header read timeout (10 s), and panic isolation (a panicking handler yields `500`). Strada does not yet terminate TLS. Put it behind a TLS-terminating proxy for internet-facing use.
