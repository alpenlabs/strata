use anyhow::Result;
use clap::{command, Parser};
use reqwest::Client;
use serde::Serialize;
use serde_json::json;
use strata_provers_perf::{ProofGeneratorPerf, ProofReport, ZkVmHostPerf};
use strata_test_utils::{bitcoin::get_btc_chain, l2::gen_params};
use strata_zkvm_tests::{CheckpointBatchInfo, TestProverGenerators, TEST_SP1_GENERATORS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sp1_sdk::utils::setup_logger();
    let args = EvalArgs::parse();

    let mut results_text = vec![format_header(&args)];

    let sp1_reports = run_generator_programs(&TEST_SP1_GENERATORS);
    //let risc0_reports = run_generator_programs(&TEST_RISC0_GENERATORS);

    results_text.push(format_results(&sp1_reports, "SP1".to_owned()));
    //results_text.push(format_results(&risc0_reports, "RISC0".to_owned()));

    // Print results
    println!("{}", results_text.join("\n"));

    if args.post_to_gh {
        // Post to GitHub PR
        let message = format_github_message(&results_text);
        post_to_github_pr(&args, &message).await?;
    }

    if !sp1_reports
        .iter()
        //.chain(risc0_reports.iter())
        .all(|r| r.success)
    {
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

    /// The GitHub repository owner.
    #[arg(long, default_value = "")]
    pub repo_owner: String,

    /// The GitHub repository name.
    #[arg(long, default_value = "")]
    pub repo_name: String,

    /// The GitHub PR number.
    #[arg(long, default_value = "")]
    pub pr_number: String,

    /// The name of the branch.
    #[arg(long, default_value = "cur_branch")]
    pub branch_name: String,

    /// The commit hash.
    #[arg(long, default_value = "local_commit")]
    pub commit_hash: String,

    /// The author of the commit.
    #[arg(long, default_value = "local")]
    pub author: String,
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

    // Init test params.
    let params = gen_params();
    let rollup_params = params.rollup();

    let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
    let l1_end_height = l1_start_height + 1;

    let l2_start_height = 1;
    let l2_end_height = 3;

    let btc_block_id = 40321;
    let btc_chain = get_btc_chain();
    let btc_block = btc_chain.get_block(btc_block_id);
    let strata_block_id = 1;

    // btc_blockspace
    println!("Generating a report for BTC_BLOCKSPACE");
    let btc_blockspace = generator.btc_blockspace();
    let btc_blockspace_report = btc_blockspace
        .gen_proof_report(btc_block, "BTC_BLOCKSPACE".to_owned())
        .unwrap();

    reports.push(btc_blockspace_report.into());

    // el_block
    println!("Generating a report for EL_BLOCK");
    let el_block = generator.el_block();
    let el_block_report = el_block
        .gen_proof_report(&strata_block_id, "EL_BLOCK".to_owned())
        .unwrap();

    reports.push(el_block_report.into());

    // cl_block
    println!("Generating a report for CL_BLOCK");
    let cl_block = generator.cl_block();
    let cl_block_report = cl_block
        .gen_proof_report(&strata_block_id, "CL_BLOCK".to_owned())
        .unwrap();

    reports.push(cl_block_report.into());

    // l1_batch
    println!("Generating a report for L1_BATCH");
    let l1_batch = generator.l1_batch();
    let l1_batch_report = l1_batch
        .gen_proof_report(&(l1_start_height, l1_end_height), "L1_BATCH".to_owned())
        .unwrap();

    reports.push(l1_batch_report.into());

    // l2_block
    println!("Generating a report for L2_BATCH");
    let l2_block = generator.l2_batch();
    let l2_block_report = l2_block
        .gen_proof_report(&(l2_start_height, l2_end_height), "L2_BATCH".to_owned())
        .unwrap();

    reports.push(l2_block_report.into());

    // checkpoint
    println!("Generating a report for CHECKPOINT");
    let checkpoint = generator.checkpoint();
    let checkpoint_test_input = CheckpointBatchInfo {
        l1_range: (l1_start_height.into(), l1_end_height.into()),
        l2_range: (l2_start_height, l2_end_height),
    };
    let checkpoint_report = checkpoint
        .gen_proof_report(&checkpoint_test_input, "CHECKPOINT".to_owned())
        .unwrap();
    reports.push(checkpoint_report.into());

    reports
}

/// Returns a formatted header for the performance report with basic PR data.
fn format_header(args: &EvalArgs) -> String {
    let mut detail_text = String::new();

    if args.post_to_gh {
        detail_text.push_str(&format!("*Branch*: {}\n", &args.branch_name));
        detail_text.push_str(&format!("*Commit*: {}\n", &args.commit_hash[..8]));
        detail_text.push_str(&format!("*Author*: {}\n", &args.author));
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
            result.cycles,
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
    let base_url = format!(
        "https://api.github.com/repos/{}/{}",
        &args.repo_owner, &args.repo_name
    );

    // Get all comments on the PR
    let comments_url = format!("{}/issues/{}/comments", base_url, &args.pr_number);
    let comments_response = client
        .get(&comments_url)
        .header("Authorization", format!("token {}", &args.github_token))
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
            .header("Authorization", format!("token {}", &args.github_token))
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
            .header("Authorization", format!("token {}", &args.github_token))
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
