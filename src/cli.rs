// Copyright (C) 2021-2021 Fuwn
// SPDX-License-Identifier: GPL-3.0-only

use std::str::FromStr;

use structopt::{
    clap,
    clap::{App, Arg, SubCommand},
};

use crate::nitrous::Nitrous;

#[derive(PartialEq)]
pub enum ProxyType {
    Http,
    Socks4,
    Socks5,
    Tor,
}
impl FromStr for ProxyType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(Self::Http),
            "socks4" => Ok(Self::Socks4),
            "socks5" => Ok(Self::Socks5),
            "tor" => Ok(Self::Tor),
            _ => Err("no match"),
        }
    }
}

pub struct Cli;
impl Cli {
    pub async fn execute() {
        let matches = Self::cli().get_matches();

        let debug = matches.is_present("debug");

        match matches.subcommand() {
            ("generate", _) => Nitrous::generate(
                matches
                    .subcommand_matches("generate")
                    .unwrap()
                    .value_of("amount")
                    .unwrap()
                    .to_string()
                    .parse::<usize>()
                    .unwrap(),
                debug,
            ),
            ("check", _) => {
                let sub_matches = matches.subcommand_matches("check").unwrap();
                let concurrency = sub_matches
                    .value_of("concurrency")
                    .unwrap_or("10") // Default concurrency
                    .parse::<usize>()
                    .expect("Concurrency must be a number");

                Nitrous::check(
                    {
                        let argument = sub_matches.value_of("file");
                        if argument.is_some() {
                            argument.unwrap()
                        } else if std::fs::File::open(".nitrous/codes.txt").is_err() {
                            panic!("cannot open nitrous generated codes.txt");
                        } else {
                            ".nitrous/codes.txt"
                        }
                    },
                    debug,
                    ProxyType::from_str(
                        sub_matches
                            .value_of("proxy_type")
                            .unwrap(),
                    )
                    .unwrap(),
                    sub_matches
                        .value_of("proxy_list")
                        .unwrap_or("null"),
                    concurrency, // Pass concurrency to the check method
                )
                .await;
            }
            ("clean", _) => for dir in &[".nitrous/check/", ".nitrous/"] {
                let file_type = if dir.ends_with('/') {
                    "directory"
                } else {
                    "file"
                };
                info!("cleaning {}: {}", file_type, dir);
                if let Err(e) = std::fs::remove_dir_all(dir) {
                    warn!("cannot delete {}: {}: {}", file_type, dir, e);
                }
            },
            _ => unreachable!(),
        }
    }

    fn cli() -> App<'static, 'static> {
        App::new(env!("CARGO_PKG_NAME"))
            .about(env!("CARGO_PKG_DESCRIPTION"))
            .version(env!("CARGO_PKG_VERSION"))
            .author(env!("CARGO_PKG_AUTHORS"))
            .setting(clap::AppSettings::SubcommandRequiredElseHelp)
            .subcommands(vec![
                SubCommand::with_name("generate")
                    .alias("gen")
                    .about("Generate a specified number Discord Nitro codes")
                    .arg(
                        Arg::with_name("amount")
                            .required(true)
                            .index(1)
                            .takes_value(true),
                    ),
                SubCommand::with_name("check")
                    .about("Check a file of Discord Nitro codes for valid/ invalid codes")
                    .long_about(
                        "Check a file of Discord Nitro codes for valid/ invalid codes.\n\nIf a codes file is \
                         not explicitly specified, the check routine will run on a default file value of \
                         `./.nitrous/codes.txt`. If you would like to override this behaviour, specify your \
                         file after the subcommand.",
                    )
                    .arg(
                        Arg::with_name("file")
                            .required(false)
                            .takes_value(true)
                            .long("file")
                            .short("f"),
                    )
                    .arg(
                        Arg::with_name("concurrency")
                            .long("concurrency")
                            .short("c")
                            .takes_value(true)
                            .default_value("10")
                            .help("Set the concurrency limit for code checking"),
                    )
                    .args(&[
                        Arg::with_name("proxy_type")
                            .required(true)
                            .takes_value(true)
                            .index(1)
                            .possible_values(&["http", "socks4", "socks5", "tor"]),
                        Arg::with_name("proxy_list")
                            .required_ifs(&[
                                ("proxy_type", "http"),
                                ("proxy_type", "socks4"),
                                ("proxy_type", "socks5"),
                            ])
                            .takes_value(true)
                            .index(2),
                    ]),
                SubCommand::with_name("clean")
                    .about("Delete Nitrous-generated files/ directories which are NOT critical."),
            ])
            .arg(
                Arg::with_name("debug")
                    .long("debug")
                    .short("d")
                    .takes_value(false)
                    .multiple(false)
                    .global(true),
            )
    }
}
