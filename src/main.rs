use std::env::set_var;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use wasp_app_route::start;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "wasp",
    raw(global_settings = "&[AppSettings::ColoredHelp, AppSettings::VersionlessSubcommands]")
)]
enum Opt {
    #[structopt(name = "run")]
    /// Run a wasp module locally
    Run {
        #[structopt(name = "MODULE")]
        module: String,

        #[structopt(short = "f", long = "function", default_value = "run")]
        function: String,

        #[structopt(short = "p", long = "port", default_value = "5000")]
        port: usize,

        #[structopt(short = "e", long = "env-file")]
        env_file: Option<String>,
    },

    #[structopt(name = "deploy")]
    Deploy {
        // TODO
    },

    #[structopt(name = "signup")]
    Signup {
        // TODO
    },

    #[structopt(name = "login")]
    Login {
        // TODO
    },

    #[structopt(name = "logout")]
    Logout {
        // TODO
    },
}

fn main() {
    match Opt::from_args() {
        Opt::Run {
            module,
            function,
            port,
            env_file,
        } => run(module, function, port, env_file),
        _ => unimplemented!(),
    }
}

fn run(module: String, function: String, port: usize, env_file: Option<String>) {
    if let Some(file) = env_file {
        dotenv::from_filename(file).expect("Could not load env file");
    }

    set_var("WASP_PLATFORM_FILE", module);
    set_var("WASP_PLATFORM_ENTRY_FUNCTION", function);
    set_var("PORT", port.to_string());

    start();
}
