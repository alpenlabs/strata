import hashlib

import flexitest
from strata_utils import sign_schnorr_sig
from web3 import Web3

from envs import testenv
from utils import wait_until_with_value
from utils.constants import PRECOMPILE_SCHNORR_ADDRESS


@flexitest.register
class SchnorrPrecompileTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        """
        Schnorr Precompile is available at address
        `0x5400000000000000000000000000000000000002`

        The format required is concatenation of
        `public_key` , `message_hash` and `schnorr signature` in order

        This test checks for the valid and invalid input for this precompile
        """
        reth = ctx.get_service("reth")
        self.web3: Web3 = reth.create_web3()

        self.source = self.web3.address
        self.dest = self.web3.to_checksum_address(PRECOMPILE_SCHNORR_ADDRESS)

        # secret key
        secret_key = "a9f913c3d7fe56c462228ad22bb7631742a121a6a138d57c1fc4a351314948fa"
        self.debug(secret_key)

        message_hash = hashlib.sha256(b"AlpenStrata").hexdigest()
        (signature, public_key) = sign_schnorr_sig(message_hash, secret_key)
        signature = signature.hex()
        public_key = public_key.hex()

        valid_precompile_input = public_key + message_hash + signature
        data = self.wait_for_precompile_response(valid_precompile_input)
        assert data == "0x01", f"Schnorr verification failed: expected '0x01', got '{data}'."

        invalid_message = hashlib.sha256(b"MakaluStrata").hexdigest()
        invalid_precompile_input = public_key + invalid_message + signature
        data = self.wait_for_precompile_response(invalid_precompile_input)
        assert data == "0x00", f"Schnorr verification failed: expected '0x00', got '{data}'."

        return True

    def wait_for_precompile_response(self, precompile_input: str):
        assert self.web3.is_connected(), "cannot connect to reth"
        txid = self.schnorr_precompile(precompile_input)

        receipt = wait_until_with_value(
            lambda: self.web3.eth.get_transaction_receipt(txid),
            lambda x: not isinstance(x, Exception),
            error_with="Transaction receipt for txid not available",
        )

        assert receipt.status == 1, "precompile transaction failed"

        data = self.web3.eth.call(
            {
                "to": self.dest,
                "data": precompile_input,
            }
        )
        return data.to_0x_hex()

    def schnorr_precompile(self, data: str):
        # No Wei needs to be sent for this precompile
        to_transfer_wei = 0

        txid = self.web3.eth.send_transaction(
            {
                "to": self.dest,
                "value": hex(to_transfer_wei),
                "gas": hex(100000),
                "from": self.source,
                "data": data,
            }
        )
        return txid
