use gettextrs::{
    bind_textdomain_codeset,
    bindtextdomain,
    gettext,
    ngettext,
    setlocale,
    textdomain,
    LocaleCategory,
};

const GETTEXT_PACKAGE: &str = "galaxybook-setup";
const SYSTEM_LOCALE_DIR: &str = "/usr/share/locale";

pub fn init_i18n() {
    let _ = setlocale(LocaleCategory::LcAll, "");
    let _ = bindtextdomain(GETTEXT_PACKAGE, SYSTEM_LOCALE_DIR);
    let _ = bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8");
    let _ = textdomain(GETTEXT_PACKAGE);
}

pub fn tr(message: &str) -> String {
    gettext(message)
}

pub const fn tr_mark(message: &'static str) -> &'static str {
    message
}

pub fn trn(singular: &str, plural: &str, value: u32) -> String {
    ngettext(singular, plural, value)
}

pub fn trf(message: &str, replacements: &[(&str, String)]) -> String {
    let mut translated = gettext(message);
    for (key, value) in replacements {
        translated = translated.replace(&format!("{{{key}}}"), value);
    }
    translated
}
