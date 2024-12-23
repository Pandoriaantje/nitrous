// Copyright (C) 2021-2021 Fuwn
// SPDX-License-Identifier: GPL-3.0-only

use std::fs::create_dir;
use std::sync::{Arc, Once};
use rand::{seq::SliceRandom, Rng};
use rayon::prelude::*;
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Semaphore;
use reqwest::{Client, Proxy};
use futures::stream::{FuturesUnordered, StreamExt};
use std::io::Write;
use std::time::Instant;
use tracing::{info, error};
use tracing_subscriber;
use human_panic::setup_panic;

use crate::cli::ProxyType;

static INIT: Once = Once::new();

pub struct Nitrous;

impl Nitrous {
    pub async fn execute() {
        // Initialize the logger safely
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt::try_init(); // Safely initialize tracing
            let _ = pretty_env_logger::try_init();      // Safely initialize pretty_env_logger
        });

        dotenv::dotenv().ok();
        std::env::set_var("RUST_LOG", "nitrous=trace");
        setup_panic!();

        crate::cli::Cli::execute().await;
    }

    pub fn initialize() {
        let _ = create_dir(".nitrous");
    }

    pub fn generate(amount: usize, debug: bool) {
        Self::initialize();

        let mut codes_file = std::fs::File::create(".nitrous/codes.txt")
            .expect("Failed to create codes file");

        let codes: Vec<String> = (0..amount)
            .into_par_iter()
            .map(|_| {
                rand::thread_rng()
                    .sample_iter(rand::distributions::Alphanumeric)
                    .take(16)
                    .map(char::from)
                    .collect::<String>()
            })
            .collect();

        codes.iter().for_each(|code| {
            writeln!(codes_file, "{}", code).unwrap();

            if debug {
                println!("Generated code: {}", code);
            }
        });
    }

    pub async fn check(
        codes_file_name: &str,
        debug: bool,
        proxy_type: ProxyType,
        proxy_file: &str,
        concurrency: usize, // New parameter for concurrency
    ) {
        Self::initialize();

        let _ = create_dir(".nitrous/check/");
        let codes_file = TokioFile::open(codes_file_name)
            .await
            .expect("Failed to open codes file");

        let proxies_file = TokioFile::open(proxy_file)
            .await
            .expect("Failed to open proxy file");

        let invalid = Arc::new(tokio::sync::Mutex::new(
            std::fs::File::create(".nitrous/check/invalid.txt").expect("Failed to create invalid file"),
        ));
        let valid = Arc::new(tokio::sync::Mutex::new(
            std::fs::File::create(".nitrous/check/valid.txt").expect("Failed to create valid file"),
        ));

        // Read and process proxies
        let mut proxies = Vec::new();
        let mut proxies_stream = BufReader::new(proxies_file).lines();
        while let Some(line) = proxies_stream.next_line().await.expect("Error reading proxies") {
            proxies.push(line);
        }

        let proxies = Arc::new(proxies);

        // Read and process codes
        let mut codes = Vec::new();
        let mut codes_stream = BufReader::new(codes_file).lines();
        while let Some(line) = codes_stream.next_line().await.expect("Error reading codes") {
            codes.push(line);
        }

        let start = Instant::now();
        let semaphore = Arc::new(Semaphore::new(concurrency)); // Use the concurrency parameter here

        let tasks: FuturesUnordered<_> = codes
            .into_iter()
            .map(|code| {
                let proxies = proxies.clone();
                let semaphore = semaphore.clone();
                let valid = valid.clone();
                let invalid = invalid.clone();
                let proxy_type = &proxy_type;

                async move {
                    let _permit = semaphore.acquire().await;
                    let proxy = proxies
                        .choose(&mut rand::thread_rng())
                        .expect("No proxies available");

                    let client = Client::builder()
                        .proxy(
                            Proxy::all(format!(
                                "{}://{}",
                                match *proxy_type {
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
                            if *proxy_type == ProxyType::Http { "http" } else { "https" },
                            code
                        ))
                        .send()
                        .await
                        .map(|res| res.status().as_u16())
                        .unwrap_or(0);

                    if status == 200 {
                        let mut valid = valid.lock().await;
                        writeln!(valid, "{}", code).unwrap();
                        if debug {
                            info!("Valid: {} via {}", code, proxy);
                        }
                    } else {
                        let mut invalid = invalid.lock().await;
                        writeln!(invalid, "{}", code).unwrap();
                        if debug {
                            error!("Invalid: {} via {}", code, proxy);
                        }
                    }

                    status
                }
            })
            .collect();

        let mut tasks = tasks;

        let mut valid_count = 0;
        let mut invalid_count = 0;

        while let Some(status) = tasks.next().await {
            if status == 200 {
                valid_count += 1;
            } else {
                invalid_count += 1;
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
