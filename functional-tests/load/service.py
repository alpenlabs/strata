import os

import flexitest
import gevent
from locust import events
from locust.env import Environment, LocalRunner
from locust.log import setup_logging
from locust.stats import stats_history, stats_printer

from load.cfg import LoadConfig


# TODO(load): enhance it to be able to increase/decrease the load from test runtime.
class LoadGeneratorService(flexitest.Service):
    """
    A separate flexitest service that is able to generate the load as specified by `LoadConfig`.
    """

    env: Environment
    """Locust Environment for running the load against."""

    runner: LocalRunner
    """Locust local runner that actually dispatches the load."""

    cfg: LoadConfig
    """A config that specifies load params: jobs, host, rate, etc."""

    _is_started: bool
    """Whether the LoadGenerator (as a flexitest service) is started."""

    def __init__(self, logfile, cfg: LoadConfig):
        self._is_started = False
        self.env = Environment(user_classes=cfg.jobs, events=events)
        self.runner = self.env.create_local_runner()
        self.cfg = cfg

        # TODO(load): maybe adapt it to our usual logging mechanism from the utils.
        # Right now, the format is different.
        log_level = os.getenv("LOG_LEVEL", "WARNING").upper()
        setup_logging(log_level, logfile=logfile)

    def start(self):
        self.env.events.init.fire(environment=self.env, runner=self.runner)

        gevent.spawn(stats_printer(self.env.stats))
        gevent.spawn(stats_history, self.env.runner)
        self.runner.start(len(self.cfg.jobs), spawn_rate=self.cfg.spawn_rate)
        self._is_started = True

    def stop(self):
        self._is_started = False
        self.runner.quit()

    def is_started(self):
        return self._is_started

    def check_status(self):
        return True
