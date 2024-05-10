import flexitest

@flexitest.register
class HelloTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        # TODO
        print("hello!")
