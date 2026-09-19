//! `cargo install susbot` installs the sus.bot command-line tool. The code
//! lives in the susbot-cli crate; this crate only provides the short name.

fn main() -> std::process::ExitCode {
    susbot_cli::run()
}
