FROM --platform=linux/amd64 rust:latest AS builder

WORKDIR /app
# Set environment variables for optimized release builds
ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always

# Install system dependencies
RUN apt-get update && apt-get -y upgrade && apt-get install -y \
    pkg-config \
    ca-certificates \
    clang \
    libssl-dev \
    pkg-config \
    git \
    dialog \
    xz-utils \
    build-essential \
    curl \
    libclang-dev

RUN curl -L https://sp1.succinct.xyz | bash && ~/.sp1/bin/sp1up --c-toolchain
ENV PATH="/root/.sp1/bin:$PATH"
RUN cargo prove --version
RUN ls ~/.sp1/riscv/
RUN RUSTUP_TOOLCHAIN=succinct cargo --version
ENV CC_riscv32im_succinct_zkvm_elf="/root/.sp1/riscv/riscv32im-linux-x86_64/bin/riscv32-unknown-elf-gcc"
RUN curl -LO "https://github.com/xpack-dev-tools/riscv-none-elf-gcc-xpack/releases/download/v14.2.0-2/xpack-riscv-none-elf-gcc-14.2.0-2-linux-x64.tar.gz"
RUN tar xzvf xpack-riscv-none-elf-gcc-14.2.0-2-linux-x64.tar.gz && mv xpack-riscv-none-elf-gcc-14.2.0-2/ riscv/
RUN mv riscv/ /opt

COPY . .
# Build dependencies in release mode

RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=private,target=/app/target \
    CC_riscv32im_succinct_zkvm_elf="/root/.sp1/riscv/riscv32im-linux-x86_64/bin/riscv32-unknown-elf-gcc" cargo build --release --bin strata-prover-client --features="prover"


RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=private,target=/app/target \
    cp /app/target/release/strata-prover-client /app/strata-prover-client

FROM --platform=linux/amd64 ubuntu:24.04 AS runtime
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    curl \
    iproute2 \
    net-tools \
    libssl-dev \
    libffi-dev \
    software-properties-common && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Copy the built binaries and the entrypoint script from the builder stage
COPY --from=builder /app/strata-prover-client /usr/local/bin/strata-prover-client
COPY ./docker/prover-client/entrypoint.sh entrypoint.sh

EXPOSE 9851

# Make the entrypoint script executable
RUN chmod +x entrypoint.sh

# Set the entrypoint
ENTRYPOINT ["/app/entrypoint.sh"]
