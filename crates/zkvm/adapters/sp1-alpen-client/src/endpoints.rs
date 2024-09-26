enum Endpoint {
    ProofStatus,
    UploadELF,
    UploadProvingTask,
}

impl Endpoint {
    fn url(&self) -> &str {
        match self {
            Endpoint::ProofStatus => "https://api.example.com/prove",
            Endpoint::UploadELF => "https://api.example.com/verify",
            Endpoint::UploadProvingTask => "https://api.example.com/verify",
        }
    }
}
