use std::str::FromStr;

use clap::ValueEnum;

mod btc_blockscan;
mod checkpoint;
mod cl_stf;
mod evm_ee;

use crate::PerformanceReport;

#[derive(Debug, Clone, ValueEnum)]
#[non_exhaustive]
pub enum GuestProgram {
    BtcBlockscan,
    EvmEeStf,
    ClStf,
    Checkpoint,
}

impl FromStr for GuestProgram {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "btc-blockscan" => Ok(GuestProgram::BtcBlockscan),
            "evm-ee-stf" => Ok(GuestProgram::EvmEeStf),
            "cl-stf" => Ok(GuestProgram::ClStf),
            "checkpoint" => Ok(GuestProgram::Checkpoint),
            // Add more matches
            _ => Err(format!("unknown program: {}", s)),
        }
    }
}

/// Runs SP1 programs to generate reports.
///
/// Generates [`PerformanceReport`] for each invocation.
// #[cfg(feature = "sp1")]
pub fn run_sp1_programs(programs: &[GuestProgram]) -> Vec<PerformanceReport> {
    programs
        .iter()
        .map(|program| match program {
            GuestProgram::BtcBlockscan => {
                btc_blockscan::gen_perf_report(&btc_blockscan::sp1::host())
            }
            GuestProgram::EvmEeStf => evm_ee::gen_perf_report(&evm_ee::sp1::host()),
            GuestProgram::ClStf => cl_stf::gen_perf_report(
                &cl_stf::sp1::host(),
                evm_ee::proof_with_vk(&evm_ee::sp1::host()),
            ),
            GuestProgram::Checkpoint => checkpoint::gen_perf_report(
                &checkpoint::sp1::host(),
                cl_stf::proof_with_vk(&cl_stf::sp1::host(), &evm_ee::sp1::host()),
            ),
        })
        .collect()
}

/// Runs Risc0 programs to generate reports.
///
/// Generates [`PerformanceReport`] for each invocation.
#[cfg(feature = "risc0")]
pub fn run_risc0_programs(programs: &[GuestProgram]) -> Vec<PerformanceReport> {
    programs
        .iter()
        .map(|program| match program {
            GuestProgram::BtcBlockscan => {
                btc_blockscan::gen_perf_report(&btc_blockscan::risc0::host())
            }
            GuestProgram::EvmEeStf => evm_ee::gen_perf_report(&evm_ee::risc0::host()),
            GuestProgram::ClStf => cl_stf::gen_perf_report(
                &cl_stf::risc0::host(),
                evm_ee::proof_with_vk(&evm_ee::risc0::host()),
            ),
            GuestProgram::Checkpoint => checkpoint::gen_perf_report(
                &checkpoint::risc0::host(),
                cl_stf::proof_with_vk(&cl_stf::risc0::host(), &evm_ee::risc0::host()),
            ),
        })
        .collect()
}
