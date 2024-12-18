// Copyright (C) 2021-2021 Fuwn
// SPDX-License-Identifier: GPL-3.0-only

use std::fs::{create_dir, File};
use std::io::{BufRead, BufReader, Write};
use rand::Rng;
use human_panic::setup_panic;
use rayon::prelude::*;

use crate::cli::ProxyType;

pub struct Nitrous;

impl Nitrous {
    pub async fn execute() {
        // Environment setup
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

        let mut codes = File::create(".nitrous/codes.txt").unwrap();

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

        // Setup directories
        let _ = create_dir(".nitrous/check/");
        let codes = File::open(codes_file_name).unwrap();
        let mut invalid = File::create(".nitrous/check/invalid.txt").unwrap();
        let mut valid = File::create(".nitrous/check/valid.txt").unwrap();
        let mut valid_count = 0;
        let mut invalid_count = 0;

        // Read codes into a Vec
        let codes: Vec<String> = BufReader::new(codes)
            .lines()
            .filter_map(Result::ok)
            .collect();

        // Parallel processing using Rayon
        let results: Vec<_> = codes.par_iter()
            .map(|code| {
                let proxy_addr = match proxy_type {
                    ProxyType::Tor => "127.0.0.1:9050".to_string(),
                    _ => {
                        let proxies = std::fs::read_to_string(proxy_file).unwrap_or_else(|_| {
                            panic!("Unable to open file: {}", proxy_file)
                        });
                        let proxy_list: Vec<_> = proxies.lines().collect();
                        proxy_list[rand::thread_rng().gen_range(0..proxy_list.len())].to_string()
                    }
                };

                let response = check_code_with_proxy(&proxy_addr, code); // Abstracted code-checking logic
                if response.is_ok() {
                    (code.clone(), true)
                } else {
                    (code.clone(), false)
                }
            })
            .collect();

        for (code, is_valid) in results {
            if is_valid {
                writeln!(valid, "{}", code).unwrap();
                valid_count += 1;
                if debug {
                    println!("Valid: {}", code);
                }
            } else {
                writeln!(invalid, "{}", code).unwrap();
                invalid_count += 1;
                if debug {
                    println!("Invalid: {}", code);
                }
            }
        }

        println!(
            "\nFinished!\n\nValid: {}\nInvalid: {}",
            valid_count, invalid_count
        );
    }
}

// Helper function for code validation
fn check_code_with_proxy(_proxy: &str, code: &str) -> Result<(), ()> {
    // Simulated API request logic
    if code.starts_with("NITRO") {
        Ok(())
    } else {
        Err(())
    }
}
