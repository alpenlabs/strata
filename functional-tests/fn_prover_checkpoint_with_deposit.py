import time
import os

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from web3 import Web3
from web3._utils.events import get_event_data


from constants import (
    ROLLUP_PARAMS_FOR_DEPOSIT_TX,
    SEQ_PUBLISH_BATCH_INTERVAL_SECS,
    PRECOMPILE_BRIDGEOUT_ADDRESS,
)
from utils import wait_for_proof_with_time_out, wait_until

EVM_WAIT_TIME = 50 * 2
DEPOSIT_WAIT_TIME = 150
SATS_TO_WEI = 10**10

withdrawal_intent_event_abi = {
    "anonymous": False,
    "inputs": [
        {"indexed": False, "internalType": "uint64", "name": "amount", "type": "uint64"},
        {"indexed": False, "internalType": "bytes", "name": "dest_pk", "type": "bytes32"},
    ],
    "name": "WithdrawalIntentEvent",
    "type": "event",
}
event_signature_text = "WithdrawalIntentEvent(uint64,bytes32)"


@flexitest.register
class BridgeDepositTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        evm_addr = "deedf001900dca3ebeefdeadf001900dca3ebeef"

        btc = ctx.get_service("bitcoin")
        btcrpc: BitcoindClient = btc.create_rpc()

        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        """
        Depostis and withdrwals happens here
        """
        # Do deposit and collect the L1 and L2 where the deposit transaction was included
        l2_block_num, l1_deposit_txn_id = self.do_deposit(ctx, evm_addr)
        block_num_withdrawl = self.do_withdrawal_precompile_call(ctx)
        l1_deposit_txn_block_info = btcrpc.proxy.gettransaction(l1_deposit_txn_id)
        l1_deposit_txn_block_num = l1_deposit_txn_block_info["blockheight"]
        # Log the metadata
        print("Depost: L1 number: ", l1_deposit_txn_block_num, " L2 deposit number: ", l2_block_num)
        print("Withdrawl: L2 ", block_num_withdrawl)

        """
        Send the first checkpoint proof
        """
        ckp_idx_0 = seqrpc.strata_getLatestCheckpointIndex()
        ckp = seqrpc.strata_getCheckpointInfo(ckp_idx_0)
        print("The initial checkpoint range: ", ckp)
        print("Sending mock proof")
        seqrpc.strataadmin_submitCheckpointProof(0, "")
        print("Mock proof sent")
        # Wait for the new checkpoint from the sequencer
        wait_until(
            lambda: int(seqrpc.strata_getLatestCheckpointIndex() > ckp_idx_0),
            error_with="New checkpoint didn't came",
            timeout=100,
        )
        ckp_idx_1 = seqrpc.strata_getLatestCheckpointIndex()
        ckp_1 = seqrpc.strata_getCheckpointInfo(ckp_idx_1)
        l1_range = ckp_1["l1_range"]
        start, end = l1_range[0], l1_range[1]
        l1_ckp_block = self.scan_checkpoint_block(start, end, btcrpc)
        print(f"Ckp {ckp_idx_0}: {ckp} was settled in the block {l1_ckp_block}\n")

        """
        Send the second checkpoint proof
        """
        print("Sending mock proof")
        seqrpc.strataadmin_submitCheckpointProof(1, "")
        print("Mock proof sent")
        # Wait for the new checkpoint from the sequencer
        wait_until(
            lambda: int(seqrpc.strata_getLatestCheckpointIndex() > ckp_idx_1),
            error_with="New checkpoint didn't came",
            timeout=100,
        )
        ckp_idx_2 = seqrpc.strata_getLatestCheckpointIndex()
        ckp_2 = seqrpc.strata_getCheckpointInfo(ckp_idx_2)

        l1_range = ckp_2["l1_range"]
        start, end = l1_range[0], l1_range[1]
        l1_ckp_block = self.scan_checkpoint_block(start, end, btcrpc)
        print(f"Ckp {ckp_idx_1}: {ckp_1} was settled in the block {l1_ckp_block}\n")

        """
        Get the chainstate
        """
        time.sleep(10)
        end_block = ckp_1["l2_range"][1] + 1
        chain_state = seqrpc.strata_getCLBlockWitness(end_block)
        assert chain_state is not None
        print(chain_state)

        # Init the prover client
        # prover_client = ctx.get_service("prover_client")
        # prover_client_rpc = prover_client.create_rpc()
        # time.sleep(60)

        # Dispatch the prover task
        # Proving task with with few L1 and L2 blocks including the deposit transaction
        # l1_range = (deposit_txn_block_num - 1, deposit_txn_block_num + 1)
        # l2_range = (l2_block_num - 1, l2_block_num + 1)
        # task_id = prover_client_rpc.dev_strata_proveCheckpointRaw(0, l1_range, l2_range)

        # task_id = prover_client_rpc.prove_latest_checkpoint()
        # print("got proving task_id ", task_id)
        # assert task_id is not None

        # time_out = 30 * 60
        # wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)

    def do_deposit(self, ctx: flexitest.RunContext, evm_addr: str):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        amount_to_send = ROLLUP_PARAMS_FOR_DEPOSIT_TX["deposit_amount"] / 10**8
        name = ROLLUP_PARAMS_FOR_DEPOSIT_TX["rollup_name"].encode("utf-8").hex()

        addr = "bcrt1pzupt5e8eqvt995r57jmmylxlswqfddsscrrq7njygrkhej3e7q2qur0c76"
        outputs = [{addr: amount_to_send}, {"data": f"{name}{evm_addr}"}]

        options = {"changePosition": 2}

        psbt_result = btcrpc.proxy.walletcreatefundedpsbt([], outputs, 0, options)
        psbt = psbt_result["psbt"]

        signed_psbt = btcrpc.proxy.walletprocesspsbt(psbt)

        finalized_psbt = btcrpc.proxy.finalizepsbt(signed_psbt["psbt"])
        deposit_tx = finalized_psbt["hex"]

        original_num_deposits = len(seqrpc.strata_getCurrentDeposits())
        print(f"Original deposit count: {original_num_deposits}")

        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        original_balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        print(f"Balance before deposit: {original_balance}")

        btc_txn_id = btcrpc.sendrawtransaction(deposit_tx)["txid"]

        # check if we are getting deposits
        wait_until(
            lambda: len(seqrpc.strata_getCurrentDeposits()) > original_num_deposits,
            error_with="seem not be getting deposits",
            timeout=DEPOSIT_WAIT_TIME,
        )

        current_block_num = int(rethrpc.eth_blockNumber(), base=16)
        print(f"Current reth block num: {current_block_num}")

        wait_until(
            lambda: int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16) > original_balance,
            error_with="eth balance did not update",
            timeout=EVM_WAIT_TIME,
        )

        deposit_amount = ROLLUP_PARAMS_FOR_DEPOSIT_TX["deposit_amount"] * SATS_TO_WEI

        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        print(f"Balance after deposit: {balance}")

        net_balance = balance - original_balance
        assert net_balance == deposit_amount, f"invalid deposit amount: {net_balance}"

        wait_until(
            lambda: int(rethrpc.eth_blockNumber(), base=16) > current_block_num,
            error_with="not building blocks",
            timeout=EVM_WAIT_TIME * 2,
        )

        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        net_balance = balance - original_balance
        assert (
            net_balance == deposit_amount
        ), f"deposit processed multiple times, extra: {balance - original_balance - deposit_amount}"

        # Scan the L2 blocks where the deposits were included
        start_block = 1
        end_block = int(rethrpc.eth_blockNumber(), base=16) + 1
        for block_num in range(start_block, end_block):
            block = rethrpc.eth_getBlockByNumber(hex(block_num), False)
            withdrawals = block.get("withdrawals", None)
            if withdrawals is not None and len(withdrawals) != 0:
                return block_num, btc_txn_id

        return None, btc_txn_id

    def do_withdrawal_precompile_call(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")
        web3: Web3 = reth.create_web3()

        source = web3.address
        dest = web3.to_checksum_address(PRECOMPILE_BRIDGEOUT_ADDRESS)
        # 64 bytes
        dest_pk = os.urandom(32).hex()
        print("dest_pk", dest_pk)

        assert web3.is_connected(), "cannot connect to reth"

        original_block_no = web3.eth.block_number
        original_bridge_balance = web3.eth.get_balance(dest)
        original_source_balance = web3.eth.get_balance(source)

        # assert original_bridge_balance == 0

        # 10 rollup btc as wei
        to_transfer_wei = 10_000_000_000_000_000_000

        txid = web3.eth.send_transaction(
            {
                "to": dest,
                "value": hex(to_transfer_wei),
                "gas": hex(100000),
                "from": source,
                "data": dest_pk,
            }
        )

        receipt = web3.eth.wait_for_transaction_receipt(txid, timeout=5)

        assert receipt.status == 1, "precompile transaction failed"
        assert len(receipt.logs) == 1, "no logs or invalid logs"

        event_signature_hash = web3.keccak(text=event_signature_text).hex()
        log = receipt.logs[0]
        assert web3.to_checksum_address(log.address) == dest
        assert log.topics[0].hex() == event_signature_hash
        event_data = get_event_data(web3.codec, withdrawal_intent_event_abi, log)

        # 1 rollup btc = 10**18 wei
        to_transfer_sats = to_transfer_wei // 10_000_000_000

        assert event_data.args.amount == to_transfer_sats
        assert event_data.args.dest_pk.hex() == dest_pk

        final_block_no = web3.eth.block_number
        final_bridge_balance = web3.eth.get_balance(dest)
        final_source_balance = web3.eth.get_balance(source)

        assert original_block_no < final_block_no, "not building blocks"
        assert final_bridge_balance == original_bridge_balance, "bridge out funds not burned"
        total_gas_price = receipt.gasUsed * receipt.effectiveGasPrice
        assert (
            final_source_balance == original_source_balance - to_transfer_wei - total_gas_price
        ), "final balance incorrect"

        return receipt.blockNumber

    def scan_checkpoint_block(self, start, end, client):
        is_found = False
        g_block_height, g_block_hash = None, None

        for block_height in range(start, end + 1):
            block_hash = client.proxy.getblockhash(block_height)
            block = client.proxy.getblock(block_hash)
            txids = block["tx"]
            if self.scan_proof(txids, client):
                is_found = True
                g_block_height = block_height
                g_block_hash = block_hash

        if is_found:
            return (g_block_height, g_block_hash)

        raise Exception(f"Checkpoint not found in blocks {start} to {end}")

    def scan_proof(self, txids, client):
        for txid in txids:
            raw_tx = client.proxy.getrawtransaction(txid)
            decoded_tx = client.proxy.decoderawtransaction(raw_tx)

            # Check each output for Taproot scriptPubKey
            for vout in decoded_tx.get("vout", []):
                script_pub_key = vout.get("scriptPubKey", {})
                if script_pub_key.get("type") == "witness_v1_taproot":
                    return True
