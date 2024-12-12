import flexitest

from utils import setup_test_logger


class TestStrata(flexitest.Test):
    """
    Class to be used instead of TestStrata for accessing logger
    """

    def premain(self, ctx: flexitest.RunContext):
        logger = setup_test_logger(ctx.datadir_root, ctx.name)
        self.debug = logger.debug
        self.info = logger.info
        self.warning = logger.warning
        self.error = logger.error
        self.critical = logger.critical


class ExtendedTestRuntime(flexitest.TestRuntime):
    """
    Extended TestStrataRuntime to call custom run context
    """

    def create_run_context(self, name: str, env: flexitest.LiveEnv) -> flexitest.RunContext:
        return RunContext(self.datadir_root, name, env)


class RunContext(flexitest.RunContext):
    """
    Custom run context which has access to services and some test specific variables
    """

    def __init__(self, datadir_root: str, name: str, env: flexitest.LiveEnv):
        self.name = name
        self.datadir_root = datadir_root
        super().__init__(env)
