use std::time::{Duration, Instant};

use anyhow::Result;
use clap::{command, Parser};
use reqwest::Client;
use serde::Serialize;
use serde_json::json;
use strata_provers_perf::{ProofGeneratorPerf, ProofReport, ZkVmHostPerf};
use strata_test_utils::bitcoin::get_btc_chain;
use strata_zkvm_tests::{
    ProofGenerator, TestProverGenerators, TEST_NATIVE_GENERATORS, TEST_SP1_GENERATORS,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut res = evaluate_performance(&*TEST_SP1_GENERATORS, "SP1".to_owned()).await;
    //es = res.and(evaluate_performance(&*TEST_RISC0_GENERATORS, "RISC0".to_owned()).await);
    res = res.and(evaluate_performance(&*TEST_NATIVE_GENERATORS, "NATIVE".to_owned()).await);
    res
}

pub async fn evaluate_performance<H: ZkVmHostPerf>(
    generators: &TestProverGenerators<H>,
    host_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = EvalArgs::parse();
    sp1_sdk::utils::setup_logger();

    let reports = run_generator_programs(generators);
    // Prepare and format the results.
    let reports_len = reports.len();
    let success_count = reports.iter().filter(|r| r.success).count();
    let results_text = format_results(&args, &reports, host_name);

    // Print results
    println!("{}", results_text.join("\n"));

    // Post to GitHub PR
    match (
        &args.repo_owner,
        &args.repo_name,
        &args.pr_number,
        &args.github_token,
    ) {
        (Some(owner), Some(repo), Some(pr_number), Some(token)) => {
            let message = format_github_message(&results_text);
            post_to_github_pr(owner, repo, pr_number, token, &message).await?;
        }
        _ => {
            println!("Warning: post_to_github is true, required GitHub arguments are missing.")
        }
    }

    // Exit with an error if any programs failed.
    let all_successful = success_count == reports_len;
    if !all_successful {
        println!("Some programs failed. Please check the results above.");
        std::process::exit(1);
    }

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct PerformanceReport {
    program: String,
    cycles: u64,
    exec_khz: f64,
    core_khz: f64,
    compressed_khz: f64,
    time: f64,
    success: bool,
}

impl From<(ProofReport, String)> for PerformanceReport {
    fn from(value: (ProofReport, String)) -> Self {
        PerformanceReport {
            program: value.1,
            cycles: value.0.cycles,
            exec_khz: 0.0,
            core_khz: 0.0,
            compressed_khz: 0.0,
            time: 0.0,
            success: true,
        }
    }
}

fn run_generator_programs<H: ZkVmHostPerf>(
    generator: &TestProverGenerators<H>,
) -> Vec<PerformanceReport> {
    let mut reports = vec![];

    let btc_chain = get_btc_chain();
    let btc_blockspace_input = btc_chain.get_block(40321);

    let btc_blockspace = generator.btc_blockspace();
    let report = btc_blockspace
        .gen_proof_report(btc_blockspace_input)
        .unwrap();

    reports.push((report, btc_blockspace.get_proof_id(btc_blockspace_input)).into());

    reports
}

fn format_results(
    args: &EvalArgs,
    results: &[PerformanceReport],
    host_name: String,
) -> Vec<String> {
    let mut detail_text = String::new();
    if let Some(branch_name) = &args.branch_name {
        detail_text.push_str(&format!("*Branch*: {}\n", branch_name));
    }
    if let Some(commit_hash) = &args.commit_hash {
        detail_text.push_str(&format!("*Commit*: {}\n", &commit_hash[..8]));
    }
    if let Some(author) = &args.author {
        detail_text.push_str(&format!("*Author*: {}\n", author));
    }

    let mut table_text = String::new();
    table_text.push_str("```\n");
    table_text.push_str("| program           | cycles      | execute (mHz)  | core (kHZ)     | compress (KHz) | time   | success  |\n");
    table_text.push_str("|-------------------|-------------|----------------|----------------|----------------|--------|----------|");

    for result in results.iter() {
        table_text.push_str(&format!(
            "\n| {:<17} | {:>11} | {:>14.2} | {:>14.2} | {:>14.2} | {:>6} | {:<7} |",
            result.program,
            result.cycles,
            result.exec_khz / 1000.0,
            result.core_khz,
            result.compressed_khz,
            format_duration(result.time),
            if result.success { "✅" } else { "❌" }
        ));
    }
    table_text.push_str("\n```");

    vec![
        format!("*{} Performance Test Results*\n", host_name),
        detail_text,
        table_text,
    ]
}

pub fn time_operation<T, F: FnOnce() -> T>(operation: F) -> (T, Duration) {
    let start = Instant::now();
    let result = operation();
    let duration = start.elapsed();
    (result, duration)
}

fn calculate_khz(cycles: u64, duration: Duration) -> f64 {
    let duration_secs = duration.as_secs_f64();
    if duration_secs > 0.0 {
        (cycles as f64 / duration_secs) / 1_000.0
    } else {
        0.0
    }
}

fn format_duration(duration: f64) -> String {
    let secs = duration.round() as u64;
    let minutes = secs / 60;
    let seconds = secs % 60;

    if minutes > 0 {
        format!("{}m{}s", minutes, seconds)
    } else if seconds > 0 {
        format!("{}s", seconds)
    } else {
        format!("{}ms", (duration * 1000.0).round() as u64)
    }
}

fn format_github_message(results_text: &[String]) -> String {
    let mut formatted_message = String::new();

    if let Some(title) = results_text.first() {
        // Add an extra asterisk for GitHub bold formatting
        formatted_message.push_str(&title.replace('*', "**"));
        formatted_message.push('\n');
    }

    if let Some(details) = results_text.get(1) {
        // Add an extra asterisk for GitHub bold formatting
        formatted_message.push_str(&details.replace('*', "**"));
        formatted_message.push('\n');
    }

    if let Some(table) = results_text.get(2) {
        // Remove the triple backticks as GitHub doesn't require them for table formatting
        let cleaned_table = table.trim_start_matches("```").trim_end_matches("```");
        formatted_message.push_str(cleaned_table);
    }

    formatted_message
}

#[derive(Parser, Clone)]
#[command(about = "Evaluate the performance of SP1 on programs.")]
struct EvalArgs {
    /// The GitHub token for authentication, only used if post_to_github is true.
    #[arg(long)]
    pub github_token: Option<String>,

    /// The GitHub repository owner.
    #[arg(long)]
    pub repo_owner: Option<String>,

    /// The GitHub repository name.
    #[arg(long)]
    pub repo_name: Option<String>,

    /// The GitHub PR number.
    #[arg(long)]
    pub pr_number: Option<String>,

    /// The name of the branch.
    #[arg(long)]
    pub branch_name: Option<String>,

    /// The commit hash.
    #[arg(long)]
    pub commit_hash: Option<String>,

    /// The author of the commit.
    #[arg(long)]
    pub author: Option<String>,
}

async fn post_to_github_pr(
    owner: &str,
    repo: &str,
    pr_number: &str,
    token: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let base_url = format!("https://api.github.com/repos/{}/{}", owner, repo);

    // Get all comments on the PR
    let comments_url = format!("{}/issues/{}/comments", base_url, pr_number);
    let comments_response = client
        .get(&comments_url)
        .header("Authorization", format!("token {}", token))
        .header("User-Agent", "sp1-perf-bot")
        .send()
        .await?;

    let comments: Vec<serde_json::Value> = comments_response.json().await?;

    // Look for an existing comment from our bot
    let bot_comment = comments.iter().find(|comment| {
        comment["user"]["login"]
            .as_str()
            .map(|login| login == "github-actions[bot]")
            .unwrap_or(false)
    });

    if let Some(existing_comment) = bot_comment {
        // Update the existing comment
        let comment_url = existing_comment["url"].as_str().unwrap();
        let response = client
            .patch(comment_url)
            .header("Authorization", format!("token {}", token))
            .header("User-Agent", "sp1-perf-bot")
            .json(&json!({
                "body": message
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to update comment: {:?}", response.text().await?).into());
        }
    } else {
        // Create a new comment
        let response = client
            .post(&comments_url)
            .header("Authorization", format!("token {}", token))
            .header("User-Agent", "sp1-perf-bot")
            .json(&json!({
                "body": message
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to post comment: {:?}", response.text().await?).into());
        }
    }

    Ok(())
}
