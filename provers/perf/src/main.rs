use anyhow::Result;
use clap::{command, Parser};
use num_format::{Locale, ToFormattedString};
use reqwest::Client;
use serde::Serialize;
use serde_json::json;
use strata_provers_perf::{ProofGeneratorPerf, ProofReport, ZkVmHostPerf};
use strata_test_utils::bitcoin_mainnet_segment::BtcChainSegment;
use strata_zkvm_tests::{TestProverGenerators, TEST_SP1_GENERATORS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sp1_sdk::utils::setup_logger();
    let args = EvalArgs::parse();

    let mut results_text = vec![format_header(&args)];
    let sp1_reports = run_generator_programs(&TEST_SP1_GENERATORS);
    results_text.push(format_results(&sp1_reports, "SP1".to_owned()));

    // Print results
    println!("{}", results_text.join("\n"));

    if args.post_to_gh {
        // Post to GitHub PR
        let message = format_github_message(&results_text);
        post_to_github_pr(&args, &message).await?;
    }

    if !sp1_reports.iter().all(|r| r.success) {
        println!("Some programs failed. Please check the results above.");
        std::process::exit(1);
    }

    Ok(())
}

/// Flags for CLI invocation being parsed.
#[derive(Parser, Clone)]
#[command(about = "Evaluate the performance of SP1 on programs.")]
struct EvalArgs {
    /// Whether to post on github or run locally and only log the results.
    #[arg(long, default_value_t = false)]
    pub post_to_gh: bool,

    /// The GitHub token for authentication.
    #[arg(long, default_value = "")]
    pub github_token: String,

    /// The GitHub PR number.
    #[arg(long, default_value = "")]
    pub pr_number: String,

    /// The commit hash.
    #[arg(long, default_value = "local_commit")]
    pub commit_hash: String,
}

/// Basic data about the performance of a certain [`ZkVmProver`].
///
/// TODO: Currently, only program and cycles are used, populalate the rest
/// as part of full execution with timings reporting.
#[derive(Debug, Serialize)]
pub struct PerformanceReport {
    program: String,
    cycles: u64,
    success: bool,
}

impl From<ProofReport> for PerformanceReport {
    fn from(value: ProofReport) -> Self {
        PerformanceReport {
            program: value.report_name,
            cycles: value.cycles,
            success: true,
        }
    }
}

/// Runs all prover generators from [`TestProverGenerators`] against test inputs.
///
/// Generates [`PerformanceReport`] for each invocation.
fn run_generator_programs<H: ZkVmHostPerf>(
    generator: &TestProverGenerators<H>,
) -> Vec<PerformanceReport> {
    let mut reports = vec![];

    let btc_block_id = 40321;
    let btc_chain = BtcChainSegment::load();
    let btc_block = btc_chain.get_block_at(btc_block_id).unwrap();
    let evmee_block_range = (1, 1);

    // btc_blockspace
    println!("Generating a report for BTC_BLOCKSPACE");
    let btc_blockspace = generator.btc_blockspace();
    let btc_blockspace_report = btc_blockspace
        .gen_proof_report(&btc_block, "BTC_BLOCKSPACE".to_owned())
        .unwrap();

    reports.push(btc_blockspace_report.into());

    // el_block
    println!("Generating a report for EL_BLOCK");
    let el_block = generator.el_block();
    let el_block_report = el_block
        .gen_proof_report(&evmee_block_range, "EL_BLOCK".to_owned())
        .unwrap();

    reports.push(el_block_report.into());

    // cl_block
    println!("Generating a report for CL_BLOCK");
    let cl_block = generator.cl_block();
    let cl_block_report = cl_block
        .gen_proof_report(&evmee_block_range, "CL_BLOCK".to_owned())
        .unwrap();

    reports.push(cl_block_report.into());

    // checkpoint
    println!("Generating a report for CHECKPOINT");
    let checkpoint = generator.checkpoint();
    let checkpoint_report = checkpoint
        .gen_proof_report(&evmee_block_range, "CHECKPOINT".to_owned())
        .unwrap();
    reports.push(checkpoint_report.into());

    reports
}

/// Returns a formatted header for the performance report with basic PR data.
fn format_header(args: &EvalArgs) -> String {
    let mut detail_text = String::new();

    if args.post_to_gh {
        detail_text.push_str(&format!("*Commit*: {}\n", &args.commit_hash[..8]));
    } else {
        detail_text.push_str("*Local execution*\n");
    }

    detail_text
}

/// Returns formatted results for the [`PerformanceReport`]s shaped in a table.
fn format_results(results: &[PerformanceReport], host_name: String) -> String {
    let mut table_text = String::new();
    table_text.push('\n');
    table_text.push_str("| program           | cycles      | success  |\n");
    table_text.push_str("|-------------------|-------------|----------|");

    for result in results.iter() {
        table_text.push_str(&format!(
            "\n| {:<17} | {:>11} | {:<7} |",
            result.program,
            result.cycles.to_formatted_string(&Locale::en),
            if result.success { "✅" } else { "❌" }
        ));
    }
    table_text.push('\n');

    format!("*{} Performance Test Results*\n {}", host_name, table_text)
}

/// Posts the message to the PR on the github.
///
/// Updates an existing previous comment (if there is one) or posts a new comment.
async fn post_to_github_pr(
    args: &EvalArgs,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    // Get all comments on the PR
    const BASE_URL: &str = "https://api.github.com/repos/alpenlabs/strata";
    let comments_url = format!("{}/issues/{}/comments", BASE_URL, &args.pr_number);
    let comments_response = client
        .get(&comments_url)
        .header("Authorization", format!("Bearer {}", &args.github_token))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "strata-perf-bot")
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
            .header("Authorization", format!("Bearer {}", &args.github_token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "strata-perf-bot")
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
            .header("Authorization", format!("Bearer {}", &args.github_token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "strata-perf-bot")
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

fn format_github_message(results_text: &[String]) -> String {
    let mut formatted_message = String::new();

    for line in results_text {
        formatted_message.push_str(&line.replace('*', "**"));
        formatted_message.push('\n');
    }

    formatted_message
}
