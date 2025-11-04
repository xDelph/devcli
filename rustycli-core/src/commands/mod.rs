// Module file for CLI command implementations

pub mod auto_add;
pub mod config;
pub mod monitor;
pub mod preferences;
pub mod restart;
pub mod run;
pub mod start;
pub mod status;
pub mod stop;

pub use auto_add::auto_add_command;
pub use config::{
    config_add_command, config_edit, config_edit_command, config_init, config_list,
    config_list_commands, config_remove_command, config_set_default, config_show, config_validate,
};
pub use monitor::monitor_command;
pub use preferences::{pref_reset, pref_set, pref_show};
pub use restart::restart_command;
pub use run::run_command;
pub use start::start_command;
pub use status::status_command;
pub use stop::stop_command;
