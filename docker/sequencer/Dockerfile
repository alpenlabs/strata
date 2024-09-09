FROM express_builder:builder AS builder
# Build the application binaries
RUN cargo build --profile $BUILD_PROFILE --features "$FEATURES" --locked --bin alpen-express-sequencer

# Stage 3: Final runtime environment setup
FROM ubuntu:20.04 AS runtime
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    curl \
    libssl-dev \
    libffi-dev \
    && rm -rf /var/lib/apt/lists/*


# Copy the built binaries from the builder stage
COPY --from=builder /app/target/$BUILD_PROFILE/alpen-express-sequencer /usr/local/bin/alpen-express-sequencer

# Expose necessary ports
# EXPOSE 30303 30303/udp 9001 8545 8546

ENTRYPOINT ["./docker/sequencer/entrypoint.sh"]
