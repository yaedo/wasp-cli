use failure::{format_err, Error};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{
    collections::HashMap,
    env::set_var,
    time::{Duration, SystemTime},
};
use structopt::{clap::AppSettings, StructOpt};
use wasp_app_route::start;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "wasp",
    raw(global_settings = "&[AppSettings::ColoredHelp, AppSettings::VersionlessSubcommands]")
)]
enum Opt {
    /// Run a wasp module locally
    #[structopt(name = "run")]
    Run {
        #[structopt(name = "MODULE")]
        module: String,

        #[structopt(short = "f", long = "function", default_value = "run")]
        function: String,

        #[structopt(short = "p", long = "port", default_value = "5000")]
        port: usize,

        #[structopt(short = "e", long = "env-file")]
        env_file: Option<String>,

        #[structopt(short = "c", long = "cdn-directory")]
        cdn_directory: Option<String>,

        #[structopt(short = "P", long = "protected-cdn-directory")]
        protected_cdn_directory: Option<String>,

        #[structopt(short = "k", long = "kvs-directory", default_value = ".db")]
        kvs_directory: String,
    },

    /// Upload a WASM module
    #[structopt(name = "upload")]
    Upload {
        #[structopt(name = "MODULE_PATH")]
        module: String,

        #[structopt(flatten)]
        source: SourceOpts,
    },

    /// Create a host
    #[structopt(name = "host:create")]
    Create {
        #[structopt(name = "HOST")]
        host: String,

        #[structopt(name = "CUSTOMER_ID")]
        customer_id: String,

        #[structopt(flatten)]
        configuration: ConfigureOpts,

        #[structopt(flatten)]
        source: SourceOpts,
    },

    /// Configure a host
    #[structopt(name = "host:update")]
    Configure {
        #[structopt(name = "HOST")]
        host: String,

        #[structopt(flatten)]
        configuration: ConfigureOpts,

        #[structopt(flatten)]
        source: SourceOpts,
    },

    /// View a host
    #[structopt(name = "host:get")]
    View {
        #[structopt(name = "HOST")]
        host: String,

        #[structopt(flatten)]
        source: SourceOpts,
    },

    /// Login to wasp
    #[structopt(name = "login")]
    Login {
        #[structopt(name = "USERNAME")]
        username: String,

        #[structopt(flatten)]
        source: SourceOpts,
    },

    /// Remove local wasp credentials
    #[structopt(name = "logout")]
    Logout {
        #[structopt(flatten)]
        source: SourceOpts,
    },
}

#[derive(Debug, StructOpt)]
struct ConfigureOpts {
    #[structopt(short = "m", long = "module")]
    module: Option<String>,

    #[structopt(short = "f", long = "function")]
    function: Option<String>,

    // TODO args
    // TODO log drains
    #[structopt(short = "e", long = "env", parse(try_from_str = "parse_env"))]
    env: Vec<(String, JsonValue)>,
}

#[derive(Debug, StructOpt)]
struct SourceOpts {
    #[structopt(short = "a", long = "api", default_value = "https://api.wasp.ws")]
    api: String,

    #[structopt(short = "A", long = "account", default_value = "default")]
    account: String,
}

fn parse_env(input: &str) -> Result<(String, JsonValue), String> {
    let mut parts = input.split('=');
    let name = parts
        .next()
        .ok_or_else(|| "Invalid env".to_owned())?
        .to_owned();
    let value = if let Some(v) = parts.next() {
        if v.is_empty() {
            JsonValue::Null
        } else {
            JsonValue::String(v.to_owned())
        }
    } else {
        JsonValue::String(std::env::var(&name).map_err(|_| format!("{} not found", &name))?)
    };
    Ok((name, value))
}

fn main() {
    let _ = match Opt::from_args() {
        Opt::Run {
            module,
            function,
            port,
            env_file,
            cdn_directory,
            protected_cdn_directory,
            kvs_directory,
        } => run(
            module,
            function,
            port,
            env_file,
            cdn_directory,
            protected_cdn_directory,
            kvs_directory,
        ),
        Opt::Upload { source, module } => upload(source.into(), module),
        Opt::Create {
            source,
            host,
            customer_id,
            configuration,
        } => create(source.into(), host, customer_id, configuration),
        Opt::Configure {
            source,
            host,
            configuration,
        } => configure(source.into(), host, configuration),
        Opt::View { source, host } => view(source.into(), host),
        Opt::Login { source, username } => login(source, username),
        Opt::Logout { source } => logout(source.into()),
    }
    .map_err(|err| {
        eprintln!("{}", err);
        std::process::exit(1);
    });
}

fn run(
    module: String,
    function: String,
    port: usize,
    env_file: Option<String>,
    cdn_directory: Option<String>,
    protected_cdn_directory: Option<String>,
    kvs_directory: String,
) -> Result<(), Error> {
    if let Some(file) = env_file {
        dotenv::from_filename(file).expect("Could not load env file");
    }

    set_var("WASP_PLATFORM_FILE", module);
    set_var("WASP_PLATFORM_ENTRY_FUNCTION", function);
    set_var("WASP_PLATFORM_KVS_DIR", kvs_directory);
    set_var("PORT", port.to_string());

    if let Some(dir) = cdn_directory {
        set_var("WASP_CDN_DIRECTORY", dir);
    }

    if let Some(dir) = protected_cdn_directory {
        set_var("WASP_PROTECTED_CDN_DIRECTORY", dir);
    }

    start();
    Ok(())
}

fn login(source: SourceOpts, username: String) -> Result<(), Error> {
    let password = rpassword::prompt_password_stderr("Password: ").unwrap();

    let mut response = reqwest::Client::builder()
        .timeout(None)
        .build()?
        .post(&format!("{}/login", source.api))
        .basic_auth(username, Some(password))
        .send()?;

    handle_error("Login error: ", &mut response)?;

    #[derive(Debug, Deserialize)]
    struct LoginResponse {
        access_token: String,
        expires_in: u64,
    }

    let res: LoginResponse = response.json()?;
    let keyring: Client = source.into();
    keyring.set(res.access_token, res.expires_in)?;

    eprintln!("Ok");

    Ok(())
}

fn logout(keyring: Client) -> Result<(), Error> {
    keyring.delete()?;

    Ok(())
}

struct Client {
    service: String,
    account: String,
}

impl Client {
    pub fn new(service: String, account: String) -> Self {
        Self { service, account }
    }

    fn keyring(&self) -> keyring::Keyring {
        keyring::Keyring::new(&self.service, &self.account)
    }

    pub fn set(&self, access_token: String, expires_in: u64) -> Result<(), Error> {
        self.keyring()
            .set_password(&serde_json::to_string(&KeyringEntry {
                access_token,
                expires_at: SystemTime::now() + Duration::from_secs(expires_in),
            })?)
            .map_err(|err| format_err!("{}", err))?;
        Ok(())
    }

    pub fn get_password(&self) -> Result<String, Error> {
        let entry = self.keyring().get_password().map_err(|err| match err {
            keyring::KeyringError::NoPasswordFound if self.account == "default" => {
                format_err!("No account found. Log in with `wasp login USERNAME`.")
            }
            keyring::KeyringError::NoPasswordFound => format_err!(
                "No account found. Log in with `wasp login USERNAME --account {}`.",
                self.account
            ),
            _ => format_err!("{}", err),
        })?;
        let entry: KeyringEntry = serde_json::from_str(&entry)?;

        if entry.expires_at < SystemTime::now() {
            return Err(format_err!(
                "Login token is expired. Log in again with `wasp login`."
            ));
        }

        Ok(entry.access_token)
    }

    pub fn delete(&self) -> Result<(), Error> {
        self.keyring()
            .delete_password()
            .map_err(|err| format_err!("{}", err))?;
        Ok(())
    }

    pub fn client(&self) -> Result<reqwest::Client, Error> {
        let access_token = self.get_password()?;

        let client = reqwest::Client::builder()
            .timeout(None)
            .gzip(true)
            .default_headers({
                use reqwest::header::HeaderMap;
                let mut headers = HeaderMap::new();
                headers.insert("Authorization", format!("Bearer {}", access_token).parse()?);
                headers
            })
            .build()?;

        Ok(client)
    }

    pub fn url<T: std::fmt::Display>(&self, path: T) -> String {
        format!("{}{}", self.service, path)
    }

    pub fn get<T: std::fmt::Display>(&self, path: T) -> Result<reqwest::RequestBuilder, Error> {
        Ok(self.client()?.get(&self.url(path)))
    }

    pub fn post<T: std::fmt::Display>(&self, path: T) -> Result<reqwest::RequestBuilder, Error> {
        Ok(self.client()?.post(&self.url(path)))
    }
}

impl From<SourceOpts> for Client {
    fn from(source: SourceOpts) -> Self {
        Self::new(source.api, source.account)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct KeyringEntry {
    access_token: String,
    expires_at: SystemTime,
}

fn view(client: Client, host: String) -> Result<(), Error> {
    let mut response = client.get(format!("/hosts/{}", host))?.send()?;

    handle_error("", &mut response)?;

    let response: JsonValue = response.json()?;

    println!("{:#}", response);

    Ok(())
}

fn upload(client: Client, module_path: String) -> Result<(), Error> {
    let module_id = do_upload(&client, &module_path)?;
    println!("{}", module_id);
    Ok(())
}

fn create(
    client: Client,
    host: String,
    customer_id: String,
    configuration: ConfigureOpts,
) -> Result<(), Error> {
    #[derive(Debug, Serialize)]
    struct CreateBody {
        host: String,
        customer_id: String,

        #[serde(skip_serializing_if = "Option::is_none")]
        module: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        function: Option<String>,

        #[serde(skip_serializing_if = "HashMap::is_empty")]
        env: HashMap<String, JsonValue>,
    }

    let mut response = client
        .post("/hosts")?
        .json(&CreateBody {
            host,
            customer_id,
            module: maybe_upload(&client, configuration.module)?,
            function: configuration.function,
            env: configuration.env.into_iter().collect(),
        })
        .send()?;

    handle_error("", &mut response)?;

    eprintln!("Ok");

    Ok(())
}

fn configure(client: Client, host: String, configuration: ConfigureOpts) -> Result<(), Error> {
    #[derive(Debug, Default, Serialize)]
    struct ConfigureBody {
        #[serde(skip_serializing_if = "Option::is_none")]
        module: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        function: Option<String>,

        #[serde(skip_serializing_if = "HashMap::is_empty")]
        env: HashMap<String, JsonValue>,
    }

    let mut response = client
        .post(format!("/hosts/{}", host))?
        .json(&ConfigureBody {
            module: maybe_upload(&client, configuration.module)?,
            function: configuration.function,
            env: configuration.env.into_iter().collect(),
        })
        .send()?;

    handle_error("", &mut response)?;

    eprintln!("Ok");

    Ok(())
}

fn maybe_upload(client: &Client, module: Option<String>) -> Result<Option<String>, Error> {
    if let Some(module) = module {
        if std::path::Path::new(&module).exists() {
            Ok(Some(do_upload(&client, &module)?))
        } else {
            Ok(Some(module))
        }
    } else {
        Ok(None)
    }
}

fn do_upload(client: &Client, module_path: &str) -> Result<String, Error> {
    eprintln!("Uploading module: {:?}", module_path);
    let mut response = client
        .post("/compile")?
        .body(std::fs::File::open(module_path)?)
        .send()?;

    handle_error("", &mut response)?;

    #[derive(Debug, Deserialize)]
    struct LoginResponse {
        #[serde(rename = "ok")]
        module_id: String,
    }

    let res: LoginResponse = response.json()?;

    Ok(res.module_id)
}

fn handle_error(step: &str, response: &mut reqwest::Response) -> Result<(), Error> {
    if response.status().is_success() {
        return Ok(());
    }

    #[derive(Debug, Deserialize)]
    struct ErrorResponse {
        error: String,
    }

    let text = response.text()?;
    match serde_json::from_str::<ErrorResponse>(&text) {
        Ok(err) => Err(format_err!("{}{}", step, err.error)),
        _ => Err(format_err!("{}{}", step, text)),
    }
}
