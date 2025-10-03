use clap::{Arg, ArgAction, Command};
use std::env;

#[derive(Debug)]
pub enum Error {
    #[allow(dead_code)]
    ModuleNotFound(String),
}
#[derive(Debug)]
pub struct Cli {
    pub main_target: Option<String>,
    pub only_download_modules: bool,
    pub package_path: Option<String>,
    pub no_run: bool,
    pub download: bool,
    pub print_yaml: bool,
    pub test: bool,
    pub test_filter: Option<String>,
    pub var_main: Option<String>,
}

impl Cli {
    pub fn load() -> Result<Cli, Error> {
        let command = Command::new("Runtime")
            .version(env!("CARGO_PKG_VERSION"))
            .arg(
                Arg::new("main_path")
                    .help("要加载的主路径/文件")
                    .required(false)
                    .index(1)
                    .num_args(1..),
            )
            .arg(
                Arg::new("install")
                    .long("install")
                    .short('i')
                    .action(ArgAction::SetTrue)
                    .value_parser(clap::builder::BoolishValueParser::new())
                    .help("安装依赖")
                    .default_value("false"),
            )
            .arg(
                Arg::new("download")
                    .long("download")
                    .short('d')
                    .help("运行前启用模块下载")
                    .value_parser(clap::builder::BoolValueParser::new())
                    .default_value("true"),
            )
            .arg(
                Arg::new("package")
                    .long("package")
                    .help("包文件路径"),
            )
            .arg(
                Arg::new("no_run")
                    .long("no-run")
                    .short('n')
                    .help("不运行主文件")
                    .value_parser(clap::builder::BoolishValueParser::new())
                    .action(ArgAction::SetTrue)
                    .default_value("false"),
            )
            .arg(
                Arg::new("print_yaml")
                    .long("print")
                    .short('p')
                    .help("打印从目标文件生成的YAML文件")
                    .value_parser(clap::builder::BoolishValueParser::new())
                    .action(ArgAction::SetTrue)
                    .default_value("false"),
            )
            .arg(
                Arg::new("test")
                    .long("test")
                    .short('t')
                    .help("运行phlow文件中定义的测试")
                    .value_parser(clap::builder::BoolishValueParser::new())
                    .action(ArgAction::SetTrue)
                    .default_value("false"),
            )
            .arg(
                Arg::new("test_filter")
                    .long("test-filter")
                    .help("按描述筛选测试（子字符串匹配）")
                    .requires("test")
                    .value_name("FILTER"),
            )
            .arg(
                Arg::new("var_main")
                    .long("var-main")
                    .help("设置main变量值（模拟主模块输出）")
                    .value_name("VALUE"),
            );

        let matches = command.get_matches();

        let main = match matches.get_one::<String>("main_path") {
            Some(target) => Some(target.clone()),
            None => None,
        };

        let install = *matches.get_one::<bool>("install").unwrap_or(&false);
        let package_path = matches.get_one::<String>("package").map(|s| s.to_string());

        let no_run = *matches.get_one::<bool>("no_run").unwrap_or(&false);

        let download = *matches.get_one::<bool>("download").unwrap_or(&true);

        let print_yaml = *matches.get_one::<bool>("print_yaml").unwrap_or(&false);

        let test = *matches.get_one::<bool>("test").unwrap_or(&false);

        let test_filter = matches
            .get_one::<String>("test_filter")
            .map(|s| s.to_string());

        let var_main = matches.get_one::<String>("var_main").map(|s| s.to_string());

        Ok(Cli {
            main_target: main,
            only_download_modules: install,
            package_path,
            no_run,
            download,
            print_yaml,
            test,
            test_filter,
            var_main,
        })
    }
}

#[derive(Debug)]
pub enum ModuleExtension {
    Json,
    Yaml,
    Toml,
}

impl From<&str> for ModuleExtension {
    fn from(extension: &str) -> Self {
        match extension {
            "json" => ModuleExtension::Json,
            "yaml" => ModuleExtension::Yaml,
            "yml" => ModuleExtension::Yaml,
            "toml" => ModuleExtension::Toml,
            _ => ModuleExtension::Json,
        }
    }
}
    