# Security policy

ianox is **pre-alpha** software. Only the latest release receives fixes.

## Reporting a vulnerability

Please do not open a public issue for a security problem. Use GitHub's private reporting:
<https://github.com/Subdes/ianox/security/advisories/new>

Include the version, what you did, what you expected and what happened. You will get an
answer as soon as the maintainer can; there is no guaranteed response time yet.

## What ianox does with your data

- It sends your prompts, the files you attach or mention, and anything the agent reads to
  the AI providers **you** configure. That is its purpose.
- It has no telemetry and no update checks. Besides your providers it only downloads the
  public model catalogue from <https://models.dev>.
- API keys are stored as **plain text** in `~/.config/ianox/config.json`. Protect that
  file (the default permissions of your home directory apply) and do not share it.

## The agent

- Writing and editing are limited to the open project folder.
- Shell commands ask for confirmation and, when `bubblewrap` is installed, run with
  everything outside the project folder (and `/tmp`) read-only. Without it they are not
  sandboxed; the confirmation tells you.
- Reading outside the project asks for permission unless you named the path. Locations
  such as `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.kube` and ianox's own configuration always
  ask, because their contents would be sent to the model provider.
- A command you allow can still read your files and use the network. Read what you approve.

## The local gateway

When enabled it listens on `127.0.0.1:20128` by default, over plain HTTP. Set a bearer token
in Settings if other users share the machine, and do not bind it to a public address without
putting something in front of it.
