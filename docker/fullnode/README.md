# Strata Fullnode Example


## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
  - [1. Clone this Repository](#1-clone-this-repository)
  - [2. Configure Environment Variables](#2-configure-environment-variables)
  - [3. Configure Parameters](#3-configure-parameters)
  - [4. Generate Keys](#4-generate-keys)
- [Running the Services](#running-the-services)
- [Troubleshooting](#troubleshooting)

## Prerequisites

Before you begin, ensure you have met the following requirements:

- **Docker**: Make sure Docker is installed on your machine. You can download it from [here](https://www.docker.com/get-started).
- **Docker Compose**: Ensure Docker Compose is installed. It typically comes bundled with Docker Desktop.
- **Git**: To clone the repository. Download from [here](https://git-scm.com/downloads).

## Installation

Follow these steps to set up and run the project.

### 1. Clone this Repository

```bash
git clone https://github.com/alpenlabs/strata.git
cd strata/docker/fullnode
```

> **Note:** Only `fullnode` directory contents can be copied separately if building from source is not required. Adjust `docker-compose.yaml` file accordingly.

### 2. Configure Environment Variables

Create a `.env` file in the `docker/fullnode` directory of the repository. This file will hold your environment variables.

```bash
cp path/to/your/.env .env
```

**Example `.env` Content:**

```env
SIGNETCHALLENGE=signet_challenge
UACOMMENT=ua_comment
NBITS=nbits_value
ADDNODE=signet_host
SEQUENCER_HTTP=sequencer_http_url
SEQUENCER_RPC=sequencer_rpc_url
```

### 3. Configure Parameters

Place your `params.json` file in the `configs` directory.

```bash
# Copy or move your params.json to the configs directory
cp path/to/your/params.json configs/params.json
```

> **Note:** Replace `path/to/your/params.json` with the actual path to your `params.json` file.

### 4. Generate Keys

Run the script to generate the necessary keys.

  ```bash
  ./genkeys.sh
  ```

> **Note:** Ensure the script has execute permissions. You can set this using:

```bash
chmod +x genkeys.sh
```

The script will generate a `jwt.hex` file in the `configs` directory.

## Running the Services

Once you have configured the environment and generated the necessary keys, you can start the services using Docker Compose.

```bash
docker compose up -d
```

> **Note:** The images for strata-client and strata-reth can also be built from source if required.

## Troubleshooting

- **Service fails to start:**
  - Ensure all environment variables in `.env` are correctly set.
  - Verify that `params.json` is correctly placed in the `configs` directory.
  - Check Docker logs for more details:

    ```bash
    docker compose logs -f
    ```

- **Ports already in use:**
  - If any of the specified ports are already in use, either stop the conflicting service or modify the port mappings in the `docker-compose.yml` file.

- **Key Generation Issues:**
  - Ensure you have the necessary permissions to execute `genkeys.sh`.
  - Verify that the scripts are correctly generating the `jwt.hex` file in the `configs` directory.
