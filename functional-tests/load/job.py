from locust import HttpUser


class StrataLoadJob(HttpUser):
    """
    A common layer for all the load jobs in the load tests.
    """

    def on_start(self):
        super().on_start()
        # Technically, before_start and after_start can be merged.
        # It's done to separate initialization logic (aka constructor) from "run-it-once" logic.
        # Also, with that in mind, the "on_start" is a bit misleading.
        self.before_start()
        self.after_start()

    def before_start(self):
        """
        Called right before a job starts running.
        A good place for the subclass to initialize the state.
        """
        pass

    def after_start(self):
        """
        Called right before a job starts running, but after `before_start`.
        A good place for the subclass to perform some actions once (before the job actually starts).
        """
        pass
