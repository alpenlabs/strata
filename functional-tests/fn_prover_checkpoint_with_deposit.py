import time
import os

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from web3 import Web3
from web3._utils.events import get_event_data
import net_settings


from constants import (
    ROLLUP_PARAMS_FOR_DEPOSIT_TX,
    SEQ_PUBLISH_BATCH_INTERVAL_SECS,
    PRECOMPILE_BRIDGEOUT_ADDRESS,
    DEFAULT_ROLLUP_PARAMS,
)
from strata_utils import deposit_request_transaction, drain_wallet, get_address
from utils import get_bridge_pubkey, get_logger, wait_until
from entry import BasicEnvConfig


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
class ProverBridgeDepositTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        settings = net_settings.get_fast_batch_settings()
        settings.proof_timeout = 3600

        ctx.set_env(BasicEnvConfig(101, rollup_settings=settings))
        # ctx.set_env("basic")
        self.logger = get_logger("BridgeDepositHappyTest")
        # ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        btcrpc: BitcoindClient = btc.create_rpc()

        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        """
        Depostis and withdrwals happens here
        """
        reth = ctx.get_service("reth")
        web3: Web3 = reth.create_web3()
        source = web3.address
        el_address_1 = str(source).lower()[2:]
        print(el_address_1)
        addr_1 = get_address(0)

        # 1st deposit
        self.test_deposit(ctx, addr_1, el_address_1, new_address=False)
        time.sleep(1)
        self.test_deposit(ctx, addr_1, el_address_1, new_address=False)
        time.sleep(1)

        # Do deposit and collect the L1 and L2 where the deposit transaction was included
        # l2_block_num, l1_deposit_txn_id = self.do_deposit(ctx, evm_addr)
        block_num_withdrawl = self.do_withdrawal_precompile_call(ctx)
        print("Abishek its ", block_num_withdrawl)

        # Make some txn on the reth
        beneficiary_address = web3.to_checksum_address("5400000000000000000000000000000000000011")
        to_transfer = 1_000_000_000_000_000_000
        web3.eth.send_transaction(
            {
                "to": beneficiary_address,
                "value": hex(to_transfer),
                "gas": hex(100000),
                "from": source,
            }
        )

        # l1_deposit_txn_block_info = btcrpc.proxy.gettransaction(l1_deposit_txn_id)
        # l1_deposit_txn_block_num = l1_deposit_txn_block_info["blockheight"]
        # # Log the metadata
        # print("Depost: L1 number: ", l1_deposit_txn_block_num, " L2 deposit number: ", l2_block_num)
        # print("Withdrawl: L2 ", block_num_withdrawl)
        # return

        """
        Send the first checkpoint proof
        """
        ckp_idx_0 = seqrpc.strata_getLatestCheckpointIndex()
        ckp = seqrpc.strata_getCheckpointInfo(ckp_idx_0)
        print("Current checkpoint is: ", ckp_idx_0)
        print("The initial checkpoint range: ", ckp, ckp_idx_0)
        print("Sending mock proof")
        seqrpc.strataadmin_submitCheckpointProof(ckp_idx_0, "")
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
        seqrpc.strataadmin_submitCheckpointProof(ckp_idx_1, "")
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
        return
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

    def make_drt(self, ctx: flexitest.RunContext, el_address, musig_bridge_pk):
        """
        Deposit Request Transaction
        """
        # Get relevant data
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]
        seq_addr = seq.get_prop("address")

        # Create the deposit request transaction
        tx = bytes(
            deposit_request_transaction(
                el_address, musig_bridge_pk, btc_url, btc_user, btc_password
            )
        ).hex()
        # self.logger.debug(f"Deposit request tx: {tx}")

        # Send the transaction to the Bitcoin network
        txid = btcrpc.proxy.sendrawtransaction(tx)
        # self.logger.debug(f"sent deposit request with txid = {txid} for address {el_address}")
        # this transaction is not in the bitcoind wallet, so we cannot use gettransaction
        time.sleep(1)

        # time to mature DRT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

        # time to mature DT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

    def drain_wallet(self, ctx: flexitest.RunContext):
        """
        Drains the wallet to the sequencer address
        """
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]
        seq_addr = seq.get_prop("address")

        tx = bytes(drain_wallet(seq_addr, btc_url, btc_user, btc_password)).hex()

        txid = btcrpc.proxy.sendrawtransaction(tx)
        # this transaction is not in the bitcoind wallet, so we cannot use gettransaction
        time.sleep(1)
        # self.logger.debug(f"drained wallet back to sequencer, txid: {txid}")

        return txid

    def test_deposit(
        self, ctx: flexitest.RunContext, address: str, el_address: str, new_address=True
    ):
        """
        Test depositing funds into the bridge and verifying the corresponding increase in balance
        on the Strata side.
        """
        rollup_deposit_amount = DEFAULT_ROLLUP_PARAMS["deposit_amount"]

        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        reth = ctx.get_service("reth")

        # self.logger.debug(f"EL address: {el_address}")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()
        rethrpc = reth.create_rpc()

        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]

        # self.logger.debug(f"BTC URL: {btc_url}")
        # self.logger.debug(f"BTC user: {btc_user}")
        # self.logger.debug(f"BTC password: {btc_password}")

        # Get operators pubkey and musig2 aggregates it
        bridge_pk = get_bridge_pubkey(seqrpc)
        # self.logger.debug(f"Bridge pubkey: {bridge_pk}")

        seq_addr = seq.get_prop("address")
        # self.logger.debug(f"Sequencer Address: {seq_addr}")
        # self.logger.debug(f"Address: {address}")

        n_deposits_pre = len(seqrpc.strata_getCurrentDeposits())
        # self.logger.debug(f"Current deposits: {n_deposits_pre}")

        # Make sure that the el_address has zero balance
        original_balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        # self.logger.debug(f"Balance before deposit (EL address): {original_balance}")

        if new_address:
            assert original_balance == 0, "balance is not zero"
        else:
            assert original_balance > 0, "balance is zero"

        # Generate Plenty of BTC to address
        btcrpc.proxy.generatetoaddress(102, address)

        # Send DRT from Address 1 to EL Address 1
        self.make_drt(ctx, el_address, bridge_pk)
        # Make sure that the n_deposits is correct

        n_deposits_post = len(seqrpc.strata_getCurrentDeposits())
        # self.logger.debug(f"Current deposits: {n_deposits_post}")
        assert n_deposits_post == n_deposits_pre + 1, "deposit was not registered"

        # Make sure that the balance has increased
        time.sleep(0.5)
        new_balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        # self.logger.debug(f"Balance after deposit (EL address): {new_balance}")
        assert new_balance > original_balance, "balance did not increase"

        # Make sure that the balance is the default deposit amount of BTC in Strata "wei"
        assert new_balance - original_balance == rollup_deposit_amount * (
            10**10
        ), "balance is not the default rollup_deposit_amount"

        # Drain wallet back to sequencer so that we cannot use address 1 or change anymore
        self.drain_wallet(ctx)

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
