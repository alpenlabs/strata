import logging as log
import math
import os
import time
from dataclasses import dataclass
from threading import Thread
from typing import Any, Callable, TypeVar

from bitcoinlib.services.bitcoind import BitcoindClient

from constants import ERROR_CHECKPOINT_DOESNOT_EXIST


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
            log.warning(f"{ex} while generating to address {addr}")
            return


def generate_n_blocks(bitcoin_rpc: BitcoindClient, n: int):
    addr = bitcoin_rpc.proxy.getnewaddress()
    print(f"generating {n} blocks to address", addr)
    try:
        blk = bitcoin_rpc.proxy.generatetoaddress(n, addr)
        print("made blocks", blk)
    except Exception as ex:
        log.warning(f"{ex} while generating address")
        return


def wait_until(
    fn: Callable[[], Any], error_with: str = "Timed out", timeout: int = 5, step: float = 0.5
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


def check_nth_checkpoint_finalized(
    idx,
    seqrpc,
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
    syncstat = seqrpc.alp_syncStatus()

    # Wait until we find our expected checkpoint.
    checkpoint_info = wait_until_with_value(
        lambda: seqrpc.alp_getCheckpointInfo(idx),
        predicate=lambda v: v is not None,
        error_with="Could not find checkpoint info",
        timeout=3,
    )

    assert (
        syncstat["finalized_block_id"] != checkpoint_info["l2_blockid"]
    ), "Checkpoint block should not yet finalize"
    assert checkpoint_info["idx"] == idx
    checkpoint_info_next = seqrpc.alp_getCheckpointInfo(idx + 1)
    assert checkpoint_info_next is None, f"There should be no checkpoint info for {idx + 1} index"

    to_finalize_blkid = checkpoint_info["l2_blockid"]

    # Submit checkpoint if proof_timeout is not set
    if proof_timeout is None:
        submit_checkpoint(idx, seqrpc, manual_gen)
    else:
        # Just wait until timeout period instead of submitting so that sequencer submits empty proof
        time.sleep(proof_timeout)

    if manual_gen:
        # Produce l1 blocks until proof is finalized
        manual_gen.btcrpc.proxy.generatetoaddress(
            manual_gen.finality_depth + 1, manual_gen.gen_addr
        )

    # Check if finalized
    wait_until(
        lambda: seqrpc.alp_syncStatus()["finalized_block_id"] == to_finalize_blkid,
        error_with="Block not finalized",
        timeout=10,
    )


def submit_checkpoint(idx: int, seqrpc, manual_gen: ManualGenBlocksConfig | None = None):
    """
    Submits checkpoint and if manual_gen, waits till it is present in l1
    """
    last_published_txid = seqrpc.alp_l1status()["last_published_txid"]

    # Post checkpoint proof
    # NOTE: This random proof posted will fail to make blocks finalized in l2
    # once we have checkpoint verification logic implemented. Will need to
    # change the proof accordingly
    proof_hex = "00" * 256  # The expected proof size if 256 bytes

    # This is arbitrary
    seqrpc.alpadmin_submitCheckpointProof(idx, proof_hex)

    # Wait a while for it to be posted to l1. This will happen when there
    # is a new published txid in l1status
    published_txid = wait_until_with_value(
        lambda: seqrpc.alp_l1status()["last_published_txid"],
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
    This check requires that subnitting nonexistent batch proof fails
    """
    proof_hex = "00" * 256

    try:
        seqrpc.alpadmin_submitCheckpointProof(nonexistent_batch, proof_hex)
    except Exception as e:
        if hasattr(e, "code"):
            print(e)
            assert e.code == ERROR_CHECKPOINT_DOESNOT_EXIST
        else:
            print("Unexpected error occurred")
            raise e
    else:
        raise AssertionError("Expected rpc error")


def get_logger(name: str, level=log.DEBUG) -> log.Logger:
    logger = log.getLogger(name)

    if not logger.handlers:
        handler = log.StreamHandler()
        logger.setLevel(level)
        formatter = log.Formatter(
            "%(asctime)s - %(name)s - %(levelname)s - %(filename)s:%(lineno)d - %(message)s"
        )
        handler.setFormatter(formatter)

        # Add the handler to the logger
        logger.addHandler(handler)

    return logger
