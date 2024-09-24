import math
import time
from dataclasses import dataclass
from typing import Any, Callable, TypeVar

from bitcoinlib.services.bitcoind import BitcoindClient

from constants import ERROR_CHECKPOINT_DOESNOT_EXIST


def wait_until(
    healthcheck_fn: Callable[[], Any],
    error_with: str = "Timed out",
    timeout: int = 5,
    step: float = 0.5,
):
    """
    Wait until a function call is successful given a function, time step, and timeout.
    This function waits until function call returns truth value at the interval of 1 sec
    """
    for _ in range(math.ceil(timeout / step)):
        try:
            r = healthcheck_fn()
            if not r:
                raise Exception
            return r
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


def check_for_nth_checkpoint_finalization(
    idx, seqrpc, manual_gen: ManualGenBlocksConfig | None = None
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
    print(f"checkpoint info for {idx}", checkpoint_info)

    assert (
        syncstat["finalized_block_id"] != checkpoint_info["l2_blockid"]
    ), "Checkpoint block should not yet finalize"
    assert checkpoint_info["idx"] == idx
    checkpoint_info_next = seqrpc.alp_getCheckpointInfo(idx + 1)
    assert checkpoint_info_next is None, f"There should be no checkpoint info for {idx + 1} index"

    to_finalize_blkid = checkpoint_info["l2_blockid"]
    last_published_txid = seqrpc.alp_l1status()["last_published_txid"]

    # Post checkpoint proof
    proof_hex = "abcdef"
    seqrpc.alp_submitCheckpointProof(idx, proof_hex)

    if manual_gen:
        manual_gen.btcrpc.proxy.generatetoaddress(1, manual_gen.gen_addr)
        # Wait a while for it to be posted to l1. This will happen when there
        # is a new published txid in l1status
        wait_until(
            lambda: seqrpc.alp_l1status()["last_published_txid"] != last_published_txid,
            error_with="Proof was not posted to bitcoin",
            timeout=4,
        )
        # Produce l1 blocks so that proof is finalized
        manual_gen.btcrpc.proxy.generatetoaddress(
            manual_gen.finality_depth + 1, manual_gen.gen_addr
        )

    # Check if finalized
    wait_until(
        lambda: seqrpc.alp_syncStatus()["finalized_block_id"] == to_finalize_blkid,
        error_with="Block not finalized",
        timeout=10,
    )


def check_send_proof_for_non_existent_batch(seqrpc, nonexistent_batch: int):
    """
    This check requires that subnitting nonexistent batch proof fails
    """
    some_proof_hex = "abc321"
    try:
        seqrpc.alp_submitCheckpointProof(nonexistent_batch, some_proof_hex)
    except Exception as e:
        if hasattr(e, "code"):
            assert e.code == ERROR_CHECKPOINT_DOESNOT_EXIST
        else:
            print("Unexpected exception occurred", e)
            raise e
    else:
        raise AssertionError("Expected rpc error")
