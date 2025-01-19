import flexitest

from envs import testenv


@flexitest.register
class CrashDutySignBlockTest(testenv.CrashTestBase):
    def __init__(self, ctx: flexitest.InitContext):
        super().__init__(ctx)

    def get_bail_tag(self) -> str:
        return "duty_sign_block"
