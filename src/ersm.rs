// ERSM Desk: configuration fixed at build time (replica/architecture.md, change 2).
// Applied before the custom-client config so every flavour talks only to ERSM's own
// rendezvous/relay server and never offers to update itself from rustdesk.com.
pub mod support;

use base::config::keys;
use hbb_common::config;

pub const APP_NAME: &str = "ERSM Desk";
pub const RENDEZVOUS_SERVER: &str = "host.ersmgrupo.org";
// Public key of hbbs/hbbr on host.ersmgrupo.org (id_ed25519.pub). Public by design.
pub const SERVER_PUBLIC_KEY: &str = "+KVuWKGVLFsei1T9Ph0MW0J4Mln2HfBmUuueVNzhZK8=";

pub fn apply_fixed_config() {
    *config::APP_NAME.write().unwrap() = APP_NAME.to_owned();

    let mut settings = config::OVERWRITE_SETTINGS.write().unwrap();
    for (k, v) in [
        (keys::OPTION_CUSTOM_RENDEZVOUS_SERVER, RENDEZVOUS_SERVER),
        (keys::OPTION_KEY, SERVER_PUBLIC_KEY),
        (keys::OPTION_ALLOW_AUTO_UPDATE, "N"),
    ] {
        settings.insert(k.to_owned(), v.to_owned());
    }
    drop(settings);

    config::OVERWRITE_LOCAL_SETTINGS
        .write()
        .unwrap()
        .insert(keys::OPTION_ENABLE_CHECK_UPDATE.to_owned(), "N".to_owned());

    let mut builtin = config::BUILTIN_SETTINGS.write().unwrap();
    for k in [
        keys::OPTION_HIDE_SERVER_SETTINGS,
        keys::OPTION_HIDE_WEBSOCKET_SETTINGS,
    ] {
        builtin.insert(k.to_owned(), "Y".to_owned());
    }
    drop(builtin);

    support::apply_support_settings();
}

// The server is fixed above; a renamed executable ("…host=…,key=….exe") must not redirect it.
pub fn reject_exe_name_config() -> hbb_common::ResultType<()> {
    hbb_common::bail!("server settings in the executable name are ignored by {APP_NAME}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_config_points_to_ersm_server() {
        apply_fixed_config();
        assert_eq!(
            config::Config::get_rendezvous_server().split(':').next(),
            Some(RENDEZVOUS_SERVER)
        );
        assert_eq!(
            config::Config::get_option(keys::OPTION_KEY),
            SERVER_PUBLIC_KEY
        );
        assert!(!config::Config::get_bool_option(
            keys::OPTION_ALLOW_AUTO_UPDATE
        ));
        assert_eq!(*config::APP_NAME.read().unwrap(), APP_NAME);
    }

    #[test]
    fn executable_name_cannot_change_the_server() {
        assert!(reject_exe_name_config().is_err());
    }
}
