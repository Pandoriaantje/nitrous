// Copyright (C) 2021-2021 Fuwn
// SPDX-License-Identifier: GPL-3.0-only

use std::fs::create_dir;
use std::sync::Arc;
use rand::{seq::SliceRandom, Rng};
use tokio::fs::File as TokioFile;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::Semaphore;
use reqwest::{Client, Proxy};
use futures::stream::{FuturesUnordered, StreamExt};
use std::time::Instant;
use tracing::{info, error};
use tracing_subscriber;

use crate::cli::ProxyType;

pub struct Nitrous;

impl Nitrous {
    pub async fn execute() {
        // Initialize tracing and environment
        tracing_subscriber::fmt::init();
        dotenv::dotenv().ok();
        std::env::set_var("RUST_LOG", "nitrous=trace");

        // Logging and Panic Handling
        pretty_env_logger::init();
        setup_panic!();

        crate::cli::Cli::execute().await;
    }

    pub fn initialize() {
        let _ = create_dir(".nitrous");
    }

    pub fn generate(amount: usize, debug: bool) {
        Self::initialize();

        let mut codes = std::fs::File::create(".nitrous/codes.txt").unwrap();

        for _ in 0..amount {
            let code = rand::thread_rng()
                .sample_iter(rand::distributions::Alphanumeric)
                .take(16)
                .map(char::from)
                .collect::<String>();

            writeln!(codes, "{}", code).unwrap();

            if debug {
                println!("Generated code: {}", code);
            }
        }
    }

    pub async fn check(
        codes_file_name: &str,
        debug: bool,
        proxy_type: ProxyType,
        proxy_file: &str,
    ) {
        Self::initialize();

        // Create necessary directories
        let _ = create_dir(".nitrous/check/");
        let codes_file = TokioFile::open(codes_file_name)
            .await
            .expect("Failed to open codes file");

        let proxies_file = TokioFile::open(proxy_file)
            .await
            .expect("Failed to open proxy file");

        let mut invalid = std::fs::File::create(".nitrous/check/invalid.txt")
            .expect("Failed to create invalid file");
        let mut valid = std::fs::File::create(".nitrous/check/valid.txt")
            .expect("Failed to create valid file");

        // Read proxies and shuffle
        let proxies: Vec<String> = BufReader::new(proxies_file)
            .lines()
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .await;
        let mut proxies = proxies;
        proxies.shuffle(&mut rand::thread_rng());

        // Read codes
        let codes: Vec<String> = BufReader::new(codes_file)
            .lines()
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .await;

        let start = Instant::now();
        let semaphore = Arc::new(Semaphore::new(10)); // Limit concurrency to 10

        let tasks: FuturesUnordered<_> = codes
            .into_iter()
            .map(|code| {
                let proxy = proxies
                    .choose(&mut rand::thread_rng())
                    .expect("No proxies available")
                    .to_string();
                let semaphore = semaphore.clone();

                async move {
                    let _permit = semaphore.acquire().await;
                    let client = Client::builder()
                        .proxy(
                            Proxy::all(format!(
                                "{}://{}",
                                match proxy_type {
                                    ProxyType::Http => "http",
                                    ProxyType::Socks4 => "socks4",
                                    ProxyType::Socks5 | ProxyType::Tor => "socks5h",
                                },
                                proxy
                            ))
                            .expect("Invalid proxy configuration"),
                        )
                        .build()
                        .expect("Failed to build client");

                    let status = client
                        .get(format!(
                            "{}://discordapp.com/api/v6/entitlements/gift-codes/{}?with_application=false&with_subscription_plan=true",
                            if proxy_type == ProxyType::Http { "http" } else { "https" },
                            code
                        ))
                        .send()
                        .await
                        .map(|res| res.status().as_u16())
                        .unwrap_or(0);

                    (code, proxy, status)
                }
            })
            .collect();

        let mut valid_count = 0;
        let mut invalid_count = 0;

        while let Some((code, proxy, status)) = tasks.next().await {
            if status == 200 {
                writeln!(valid, "{}", code).unwrap();
                valid_count += 1;
                if debug {
                    info!("Valid: {} via {}", code, proxy);
                }
            } else {
                writeln!(invalid, "{}", code).unwrap();
                invalid_count += 1;
                if debug {
                    error!("Invalid: {} via {}", code, proxy);
                }
            }
        }

        println!(
            "\nFinished in {:?}!\n\nValid: {}\nInvalid: {}",
            start.elapsed(),
            valid_count,
            invalid_count
        );
    }
}
