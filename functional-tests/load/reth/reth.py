from load.job import StrataLoadJob

from .account import FundedAccount, GenesisAccount


# TODO(load): configure the structured logging as we do in the tests.
class BaseRethLoadJob(StrataLoadJob):
    """
    Base class for all load jobs targetting Reth.
    """

    def before_start(self):
        super().before_start()
        self.genesis_acc = GenesisAccount(self)

    def new_account(self):
        new_acc = FundedAccount(self)
        new_acc.fund_me(self.genesis_acc)
        return new_acc
