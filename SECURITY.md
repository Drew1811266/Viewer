# Security policy

## Supported version

Viewer 0.1 is the only supported line while the project is in its initial internal-release phase. Security fixes are applied to the latest 0.1 source and test build; older commits and unofficial binaries are not supported.

## Private reporting

Do not disclose a suspected vulnerability in a public GitHub issue, discussion, pull request, or chat channel. Once the public repository enables GitHub private vulnerability reporting, use **Security → Report a vulnerability**. Before that channel exists, contact the Viewer maintainers through the private team contact already used to distribute internal builds and label the message `Viewer security`.

Include the affected commit or version, macOS version, reproduction steps, expected and observed behavior, and the smallest safe proof of concept. The maintainers will acknowledge and coordinate remediation as availability permits; this project does not promise a fixed response or resolution SLA.

## Protect project data

Viewer projects may contain confidential images, prompts, product descriptions, names, and local paths. Reports must omit or redact:

- real project files, thumbnails, screenshots, prompts, or customer data;
- full usernames, home-directory paths, volume names, and file-system metadata not required to reproduce the issue;
- authentication material, signing credentials, Apple certificates, tokens, or private repository links; and
- complete crash dumps or logs until a maintainer confirms a private transfer method.

Prefer a newly created disposable project containing synthetic files. If diagnostic output is necessary, review it locally and attach only the minimum redacted excerpt.

## Security boundary

Viewer is a local-only desktop application. It intentionally has no login, telemetry, automatic crash upload, updater, application-originated network access, broad frontend filesystem access, shell access, or frontend SQL access. Project file operations are implemented by typed Rust commands, paths remain project-relative, images are delivered through session-bound opaque tokens, and Markdown HTML is sanitized before WebView rendering.
