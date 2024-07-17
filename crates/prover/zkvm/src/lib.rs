pub struct  Proof (
    pub Vec<u8>
);

pub struct ProverOptions {
    pub enable_compression: bool,
    pub use_mock_prover: bool,
    pub stark_to_snark_conversion: bool,
}

impl Default for ProverOptions {
    fn default() -> Self {
        Self {
            enable_compression: false,
            use_mock_prover: true,
            stark_to_snark_conversion: false,
        }
    }
}


pub trait ZKVMHost {
    fn init(guest_code: Vec<u8>, prover_options: ProverOptions) -> Self;

    fn prove(&self) -> anyhow::Result<Proof>;

    fn add_input<T:serde::Serialize>(&mut self, item: T);

}

pub trait ZKVMVerifier {
    fn verify(program_id:[u32; 8], proof:&Proof) -> anyhow::Result<()>;
}