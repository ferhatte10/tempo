// Addresses used across the site, in one place.

/**
 * While false (before a release), every install command and every GitHub link waits, and
 * the site says “coming soon”. True once a release exists and the source repository is
 * public.
 */
export const AVAILABLE = false;

export const GITHUB = 'https://github.com/ferhatte10/tempo';
export const ISSUES = `${GITHUB}/issues`;
export const RELEASES = `${GITHUB}/releases`;
export const INSTALL_SCRIPT_SOURCE = `${GITHUB}/blob/main/packaging/install.sh`;
export const AUTHOR_NAME = 'Ferhat Saidoun';
export const AUTHOR_SITE = 'https://ferhatsn.fr';

/** The one command: the latest release in the home folder (packaging/install.sh). */
export const INSTALL_COMMAND = 'curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh';
export const UNINSTALL_COMMAND = 'curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh -s -- --uninstall';

/** Building from source. */
export const SOURCE_DEPS = 'sudo apt install build-essential libgtk-4-dev libadwaita-1-dev gettext desktop-file-utils';
export const SOURCE_RUST = "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh";
export const SOURCE_BUILD = 'git clone https://github.com/ferhatte10/tempo && cd tempo && make && sudo make install';
export const SOURCE_REMOVE = 'sudo make uninstall';
