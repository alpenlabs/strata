import logging
import math
import os
import subprocess
import time
from dataclasses import dataclass
from threading import Thread
from typing import Any, Callable, Optional, TypeVar

from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import convert_to_xonly_pk, musig_aggregate_pks

from constants import *
from seqrpc import JsonrpcClient


def generate_jwt_secret() -> str:
    return os.urandom(32).hex()


def generate_blocks(
    bitcoin_rpc: BitcoindClient,
    wait_dur,
    addr: str,
) -> Thread:
    thr = Thread(
        target=generate_task,
        args=(
            bitcoin_rpc,
            wait_dur,
            addr,
        ),
    )
    thr.start()
    return thr


def generate_task(rpc: BitcoindClient, wait_dur, addr):
    while True:
        time.sleep(wait_dur)
        try:
            rpc.proxy.generatetoaddress(1, addr)
        except Exception as ex:
            logging.warning(f"{ex} while generating to address {addr}")
            return


def generate_n_blocks(bitcoin_rpc: BitcoindClient, n: int):
    addr = bitcoin_rpc.proxy.getnewaddress()
    print(f"generating {n} blocks to address", addr)
    try:
        blk = bitcoin_rpc.proxy.generatetoaddress(n, addr)
        print(f"made blocks {blk}")
    except Exception as ex:
        logging.warning(f"{ex} while generating address")
        return


def wait_until(
    fn: Callable[[], Any],
    error_with: str = "Timed out",
    timeout: int = 5,
    step: float = 0.5,
):
    """
    Wait until a function call returns truth value, given time step, and timeout.
    This function waits until function call returns truth value at the interval of 1 sec
    """
    for _ in range(math.ceil(timeout / step)):
        try:
            if not fn():
                raise Exception
            return
        except Exception as _:
            pass
        time.sleep(step)
    raise AssertionError(error_with)


T = TypeVar("T")


def wait_until_with_value(
    fn: Callable[..., T],
    predicate: Callable[[T], bool],
    error_with: str = "Timed out",
    timeout: int = 5,
    step: float = 0.5,
) -> T:
    """
    Similar to `wait_until` but this returns the value of the function.
    This also takes another predicate which acts on the function value and returns a bool
    """
    for _ in range(math.ceil(timeout / step)):
        try:
            r = fn()
            if not predicate(r):
                raise Exception
            return r
        except Exception as _:
            pass
        time.sleep(step)
    raise AssertionError(error_with)


@dataclass
class ManualGenBlocksConfig:
    btcrpc: BitcoindClient
    finality_depth: int
    gen_addr: str


@dataclass
class RollupParamsSettings:
    block_time_sec: int
    epoch_slots: int
    genesis_trigger: int
    message_interval: int
    proof_timeout: Optional[int] = None

    # NOTE: type annotation: Ideally we would use `Self` but couldn't use it
    # even after changing python version to 3.12
    @classmethod
    def new_default(cls) -> "RollupParamsSettings":
        return cls(
            block_time_sec=DEFAULT_BLOCK_TIME_SEC,
            epoch_slots=DEFAULT_EPOCH_SLOTS,
            genesis_trigger=DEFAULT_GENESIS_TRIGGER_HT,
            message_interval=DEFAULT_MESSAGE_INTERVAL_MSEC,
            proof_timeout=DEFAULT_PROOF_TIMEOUT,
        )


def check_nth_checkpoint_finalized(
    idx,
    seqrpc,
    prover_rpc,
    manual_gen: ManualGenBlocksConfig | None = None,
    proof_timeout: int | None = None,
):
    """
    This check expects nth checkpoint to be finalized

    Params:
        - idx: The index of checkpoint
        - seqrpc: The sequencer rpc
        - manual_gen: If we need to generate blocks manually
    """
    syncstat = seqrpc.strata_syncStatus()

    # Wait until we find our expected checkpoint.
    batch_info = wait_until_with_value(
        lambda: seqrpc.strata_getCheckpointInfo(idx),
        predicate=lambda v: v is not None,
        error_with=f"Could not find checkpoint info for index {idx}",
        timeout=3,
    )

    assert (
        syncstat["finalized_block_id"] != batch_info["l2_blockid"]
    ), "Checkpoint block should not yet finalize"
    assert batch_info["idx"] == idx
    checkpoint_info_next = seqrpc.strata_getCheckpointInfo(idx + 1)
    assert checkpoint_info_next is None, f"There should be no checkpoint info for {idx + 1} index"

    to_finalize_blkid = batch_info["l2_blockid"]

    # Submit checkpoint if proof_timeout is not set
    if proof_timeout is None:
        submit_checkpoint(idx, seqrpc, prover_rpc, manual_gen)
    else:
        # Just wait until timeout period instead of submitting so that sequencer submits empty proof
        delta = 1
        time.sleep(proof_timeout + delta)

    if manual_gen:
        # Produce l1 blocks until proof is finalized
        manual_gen.btcrpc.proxy.generatetoaddress(
            manual_gen.finality_depth + 1, manual_gen.gen_addr
        )

    # Check if finalized
    wait_until(
        lambda: seqrpc.strata_syncStatus()["finalized_block_id"] == to_finalize_blkid,
        error_with="Block not finalized",
        timeout=10,
    )


def submit_checkpoint(
    idx: int, seqrpc, prover_rpc, manual_gen: ManualGenBlocksConfig | None = None
):
    """
    Submits checkpoint and if manual_gen, waits till it is present in l1
    """
    last_published_txid = seqrpc.strata_l1status()["last_published_txid"]

    # Post checkpoint proof
    # FIXME/NOTE: Since operating in timeout mode is supported, i.e. sequencer
    # will post empty post if prover doesn't submit proofs in time, we can send
    # empty proof hex.
    # NOTE: The functional tests for verifying proofs need to provide non-empty
    # proofs
    proof_keys = prover_rpc.dev_strata_proveCheckpoint(idx)
    proof_key = proof_keys[0]
    wait_for_proof_with_time_out(prover_rpc, proof_key, time_out=100)
    proof = prover_rpc.dev_strata_getProof(proof_key)

    seqrpc.strataadmin_submitCheckpointProof(idx, proof)

    # Wait a while for it to be posted to l1. This will happen when there
    # is a new published txid in l1status
    published_txid = wait_until_with_value(
        lambda: seqrpc.strata_l1status()["last_published_txid"],
        predicate=lambda v: v != last_published_txid,
        error_with="Proof was not published to bitcoin",
        timeout=5,
    )

    if manual_gen:
        manual_gen.btcrpc.proxy.generatetoaddress(1, manual_gen.gen_addr)

        # Check it is confirmed
        wait_until(
            lambda: manual_gen.btcrpc.proxy.gettransaction(published_txid)["confirmations"] > 0,
            timeout=5,
            error_with="Published inscription not confirmed",
        )


def check_submit_proof_fails_for_nonexistent_batch(seqrpc, nonexistent_batch: int):
    """
    Requires that submitting nonexistent batch proof fails
    """
    empty_proof_receipt = {"proof": [], "public_values": []}

    try:
        seqrpc.strataadmin_submitCheckpointProof(nonexistent_batch, empty_proof_receipt)
    except Exception as e:
        if hasattr(e, "code"):
            assert e.code == ERROR_CHECKPOINT_DOESNOT_EXIST
        else:
            print("Unexpected error occurred")
            raise e
    else:
        raise AssertionError("Expected rpc error")


def check_already_sent_proof(seqrpc, sent_batch: int):
    """
    Requires that submitting proof that was already sent fails
    """
    empty_proof_receipt = {"proof": [], "public_values": []}
    try:
        # Proof for checkpoint 0 is already sent
        seqrpc.strataadmin_submitCheckpointProof(sent_batch, empty_proof_receipt)
    except Exception as e:
        assert e.code == ERROR_PROOF_ALREADY_CREATED
    else:
        raise AssertionError("Expected rpc error")


def wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=3600):
    """
    Waits for a proof task to complete within a specified timeout period.

    This function continuously polls the status of a proof task identified by `task_id` using
    the `prover_client_rpc` client. It checks the status every 2 seconds and waits until the
    proof task status is "Completed" or the specified `time_out` (in seconds) is reached.
    """

    start_time = time.time()
    while True:
        # Fetch the proof status
        proof_status = prover_client_rpc.dev_strata_getTaskStatus(task_id)
        assert proof_status is not None
        print(f"Got the proof status {proof_status}")
        if proof_status == "Completed":
            print(f"Completed the proof generation for {task_id}")
            break

        time.sleep(2)
        elapsed_time = time.time() - start_time  # Calculate elapsed time
        if elapsed_time >= time_out:
            raise TimeoutError(f"Operation timed out after {time_out} seconds.")


def generate_seed_at(path: str):
    """Generates a seed file at specified path."""
    # fmt: off
    cmd = [
        "strata-datatool",
        "-b", "regtest",
        "genxpriv",
        "-f", path
    ]
    # fmt: on

    res = subprocess.run(cmd, stdout=subprocess.PIPE)
    res.check_returncode()


def generate_seqpubkey_from_seed(path: str) -> str:
    """Generates a sequencer pubkey from the seed at file path."""
    # fmt: off
    cmd = [
        "strata-datatool",
        "-b", "regtest",
        "genseqpubkey",
        "-f", path
    ]
    # fmt: on

    with open(path) as f:
        print(f"sequencer root privkey {f.read()}")

    res = subprocess.run(cmd, stdout=subprocess.PIPE)
    res.check_returncode()
    res = str(res.stdout, "utf8").strip()
    assert len(res) > 0, "no output generated"
    print(f"SEQ PUBKEY {res}")
    return res


def generate_opxpub_from_seed(path: str) -> str:
    """Generates operate pubkey from seed at file path."""
    # fmt: off
    cmd = [
        "strata-datatool",
        "-b", "regtest",
        "genopxpub",
        "-f", path
    ]
    # fmt: on

    res = subprocess.run(cmd, stdout=subprocess.PIPE)
    res.check_returncode()
    res = str(res.stdout, "utf8").strip()
    assert len(res) > 0, "no output generated"
    return res


def generate_params(settings: RollupParamsSettings, seqpubkey: str, oppubkeys: list[str]) -> str:
    """Generates a params file from config values."""
    # fmt: off
    cmd = [
        "strata-datatool",
        "-b", "regtest",
        "genparams",
        "--name", "alpenstrata",
        "--block-time", str(settings.block_time_sec),
        "--epoch-slots", str(settings.epoch_slots),
        "--genesis-trigger-height", str(settings.genesis_trigger),
        "--seqkey", seqpubkey,
    ]
    if settings.proof_timeout is not None:
        cmd.extend(["--proof-timeout", str(settings.proof_timeout)])
    # fmt: on

    for k in oppubkeys:
        cmd.extend(["--opkey", k])

    res = subprocess.run(cmd, stdout=subprocess.PIPE)
    res.check_returncode()
    res = str(res.stdout, "utf8").strip()
    assert len(res) > 0, "no output generated"
    return res


def generate_simple_params(
    base_path: str,
    settings: RollupParamsSettings,
    operator_cnt: int,
) -> dict:
    """
    Creates a network with params data and a list of operator seed paths.

    Result options are `params` and `opseedpaths`.
    """
    seqseedpath = os.path.join(base_path, "seqkey.bin")
    opseedpaths = [os.path.join(base_path, "opkey%s.bin") % i for i in range(operator_cnt)]
    for p in [seqseedpath] + opseedpaths:
        generate_seed_at(p)

    seqkey = generate_seqpubkey_from_seed(seqseedpath)
    opxpubs = [generate_opxpub_from_seed(p) for p in opseedpaths]

    params = generate_params(settings, seqkey, opxpubs)
    print(f"Params {params}")
    return {"params": params, "opseedpaths": opseedpaths}


def broadcast_tx(btcrpc: BitcoindClient, outputs: list[dict], options: dict) -> str:
    """
    Broadcast a transaction to the Bitcoin network.
    """
    psbt_result = btcrpc.proxy.walletcreatefundedpsbt([], outputs, 0, options)
    psbt = psbt_result["psbt"]

    signed_psbt = btcrpc.proxy.walletprocesspsbt(psbt)

    finalized_psbt = btcrpc.proxy.finalizepsbt(signed_psbt["psbt"])
    deposit_tx = finalized_psbt["hex"]

    txid = btcrpc.sendrawtransaction(deposit_tx).get("txid", "")

    return txid


def get_bridge_pubkey(seqrpc) -> str:
    """
    Get the bridge pubkey from the sequencer.
    """
    # Wait until genesis
    wait_until(
        lambda: seqrpc.strata_syncStatus() is not None,
        error_with="Genesis did not happen in time",
    )
    op_pks = seqrpc.strata_getActiveOperatorChainPubkeySet()
    print(f"Operator pubkeys: {op_pks}")
    # This returns a dict with index as key and pubkey as value
    # Iterate all of them ant then call musig_aggregate_pks
    # Also since they are full pubkeys, we need to convert them
    # to X-only pubkeys.
    op_pks = [op_pks[str(i)] for i in range(len(op_pks))]
    op_x_only_pks = [convert_to_xonly_pk(pk) for pk in op_pks]
    agg_pubkey = musig_aggregate_pks(op_x_only_pks)
    return agg_pubkey


def get_bridge_pubkey_from_cfg(cfg_params) -> str:
    """
    Get the bridge pubkey from the config.
    """
    # Slight hack to convert to appropriate operator pubkey from cfg values.
    op_pks = ["02" + pk[2:] for pk in cfg_params.operator_config.get_operators_pubkeys()]
    op_x_only_pks = [convert_to_xonly_pk(pk) for pk in op_pks]
    agg_pubkey = musig_aggregate_pks(op_x_only_pks)
    return agg_pubkey


def setup_root_logger():
    """
    reads `LOG_LEVEL` from the environment. Defaults to `WARNING` if not provided.
    """
    log_level = os.getenv("LOG_LEVEL", "WARNING").upper()
    log_level = getattr(logging, log_level, logging.NOTSET)
    # Configure the root logger
    root_logger = logging.getLogger()
    root_logger.setLevel(log_level)


def setup_test_logger(datadir_root: str, test_name: str) -> logging.Logger:
    """
    Set up loggers for a list of test names, with log files in a logs directory.
    - Configures both file and stream handlers for each test logger.
    - Logs are stored in `<datadir_root>/logs/<test_name>.log`.

    Parameters:
        datadir_root (str): Root directory for logs.
        tests (list of str): List of test names to create loggers for.

    Returns:
        logging.Logger
    """
    # Create the logs directory
    log_dir = os.path.join(datadir_root, "logs")
    os.makedirs(log_dir, exist_ok=True)

    # Common formatter
    formatter = logging.Formatter(
        "%(asctime)s - %(levelname)s - %(filename)s:%(lineno)d - %(message)s"
    )
    # Set up individual loggers for each test
    logger = logging.getLogger(test_name)
    # File handler
    log_path = os.path.join(log_dir, f"{test_name}.log")
    file_handler = logging.FileHandler(log_path)
    file_handler.setFormatter(formatter)
    # Stream handler
    stream_handler = logging.StreamHandler()
    stream_handler.setFormatter(formatter)

    # Add handlers to the logger
    logger.addHandler(file_handler)
    logger.addHandler(stream_handler)

    return logger


def get_envelope_pushdata(inp: str):
    op_if = "63"
    op_endif = "68"
    op_pushbytes_33 = "21"
    op_false = "00"
    start_position = inp.index(f"{op_false}{op_if}")
    end_position = inp.index(f"{op_endif}{op_pushbytes_33}", start_position)
    op_if_block = inp[start_position + 3 : end_position]
    op_pushdata = "4d"
    pushdata_position = op_if_block.index(f"{op_pushdata}")
    # we don't want PUSHDATA + num bytes b401
    return op_if_block[pushdata_position + 2 + 4 :]


def submit_da_blob(btcrpc: BitcoindClient, seqrpc: JsonrpcClient, blobdata: str):
    _ = seqrpc.strataadmin_submitDABlob(blobdata)

    # if blob data is present in tx witness then return the transaction
    tx = wait_until_with_value(
        lambda: btcrpc.gettransaction(seqrpc.strata_l1status()["last_published_txid"]),
        predicate=lambda tx: blobdata in tx.witness_data().hex(),
        timeout=10,
    )
    return tx


def cl_slot_to_block_id(seqrpc, slot):
    """Convert L2 slot number to block ID."""
    l2_blocks = seqrpc.strata_getHeadersAtIdx(slot)
    return l2_blocks[0]["block_id"]


def el_slot_to_block_id(rethrpc, block_num):
    """Get EL block hash from block number using Ethereum RPC."""
    return rethrpc.eth_getBlockByNumber(hex(block_num), False)["hash"]
