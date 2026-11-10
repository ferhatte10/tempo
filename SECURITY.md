# Security

Tempo writes systemd unit files in your home folder and can run a command you
chose. A flaw here matters: please report it privately, not in a public issue.

## Reporting

- On GitHub: **Security › Report a vulnerability** on this repository (private).
- Or by e-mail: ferhatsnpro@gmail.com

Say what you found, how to reproduce it, and the version (shown in the About
window). You will get an answer within a few days; this is a one-person
project, so a fix can take longer. Once it is fixed, the report is credited if
you wish.

## What counts

- A file being run while Tempo only analyses it.
- Tempo changing or deleting a timer it did not create, or writing outside
  `~/.config/systemd/user` and `~/.local/share/tempo`.
- A name or a command that ends up as something else in the unit file
  (injection of a systemd directive).
- Anything that would run as root: Tempo never should.

## Versions

Only the latest release is supported.
