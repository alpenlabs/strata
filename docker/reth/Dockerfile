FROM express_builder:builder AS builder

# Build the application binaries
RUN cargo build --profile $BUILD_PROFILE --features "$FEATURES" --locked --bin alpen-express-reth

# Stage 3: Final runtime environment setup
FROM ubuntu:20.04 AS runtime
WORKDIR /app


# Copy the built binaries from the builder stage
COPY --from=builder /app/target/$BUILD_PROFILE/alpen-express-reth /usr/local/bin/alpen-express-reth

# Expose necessary ports
# EXPOSE 30303 30303/udp 9001 8545 8546

ENTRYPOINT ["./docker/reth/entrypoint.sh"]

