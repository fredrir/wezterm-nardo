use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

pub const DEFAULT_VAR: &str = "wezplug";
pub const ROLE: &str = "backend";

/// The user-var names this process writes, all derived from one base name so a
/// host running several plugin backends never sees them collide.
pub struct Vars {
    pub base: String,
    pub role: String,
    pub token: String,
}

impl Vars {
    pub fn new(base: impl Into<String>) -> Self {
        let base = base.into();
        Self {
            role: format!("{base}_role"),
            token: format!("{base}_token"),
            base,
        }
    }
}

pub fn set_user_var(name: &str, value: &str) -> String {
    format!("\x1b]1337;SetUserVar={name}={}\x07", STANDARD.encode(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_osc_1337_byte_exact() {
        assert_eq!(
            set_user_var("demo", r#"{"t":"ping"}"#),
            "\x1b]1337;SetUserVar=demo=eyJ0IjoicGluZyJ9\x07"
        );
    }

    #[test]
    fn pads_short_values() {
        assert_eq!(set_user_var("x", "a"), "\x1b]1337;SetUserVar=x=YQ==\x07");
    }

    #[test]
    fn names_derive_from_the_base_var() {
        let vars = Vars::new("nardo");
        assert_eq!(vars.base, "nardo");
        assert_eq!(vars.role, "nardo_role");
        assert_eq!(vars.token, "nardo_token");
    }

    #[test]
    fn role_var_encodes() {
        let vars = Vars::new("demo");
        assert_eq!(
            set_user_var(&vars.role, ROLE),
            "\x1b]1337;SetUserVar=demo_role=YmFja2VuZA==\x07"
        );
    }
}
