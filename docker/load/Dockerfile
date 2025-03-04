FROM rust:latest AS builder

WORKDIR /app
# Set environment variables for optimized release builds
ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always \
    TARGET=x86_64-unknown-linux-musl

# Install system dependencies
RUN apt-get update && apt-get -y upgrade && apt-get install -y \
    pkg-config \
    build-essential \
    libclang-dev \
    curl

# Set environment variables for Poetry
ENV POETRY_VERSION=2.1.1 \
    POETRY_HOME="/opt/poetry" \
    POETRY_VIRTUALENVS_IN_PROJECT=true \
    POETRY_NO_INTERACTION=1

# Install python3 
RUN curl -sSL https://install.python-poetry.org | python3 - && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Add Poetry to PATH
ENV PATH="$POETRY_HOME/bin:$PATH"

# Set the working directory inside the container
WORKDIR /app

# Copy the rest of the application code
COPY . .

WORKDIR /app/functional-tests

# Install dependencies
RUN poetry install --no-root

WORKDIR /app
COPY ./docker/load/entrypoint.sh entrypoint.sh

# Make the entrypoint script executable
RUN chmod +x entrypoint.sh

# Set the entrypoint
ENTRYPOINT ["/app/entrypoint.sh"]
