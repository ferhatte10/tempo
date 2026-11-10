// Addresses used across the site, in one place.

/**
 * While false (before a release), every install command and every GitHub link waits, and
 * the site says “coming soon”. True once a release exists and the source repository is
 * public.
 */
export const AVAILABLE = false;

export const GITHUB = 'https://github.com/ferhatte10/tempo';
export const ISSUES = `${GITHUB}/issues`;
export const AUTHOR_NAME = 'Ferhat Saidoun';
export const AUTHOR_SITE = 'https://ferhatsn.fr';

/** Building from source, the only way to install until the first release. */
export const SOURCE_DEPS = 'sudo apt install build-essential libgtk-4-dev libadwaita-1-dev gettext desktop-file-utils';
export const SOURCE_RUST = "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh";
export const SOURCE_BUILD = 'git clone https://github.com/ferhatte10/tempo && cd tempo && make && sudo make install';
export const SOURCE_REMOVE = 'sudo make uninstall';
