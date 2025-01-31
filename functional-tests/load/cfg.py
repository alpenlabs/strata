from dataclasses import dataclass

import flexitest

from load.job import StrataLoadJob


@dataclass(frozen=True)
class LoadConfig:
    """
    Config for the load service.
    """

    jobs: list[StrataLoadJob]
    """A set of jobs that emit the load towards the host."""

    host: str
    """The host that will accept load requests."""

    spawn_rate: int
    """The rate at which all the jobs will emit the load."""


class LoadConfigBuilder:
    """
    An abstract builder of the `LoadConfig`.
    """

    jobs: list[StrataLoadJob] = []
    """A set of jobs that emit the load towards the host."""

    spawn_rate: int = 10
    """The rate at which all the jobs will emit the load."""

    service_name: str | None = None
    """The name of the service to emit the load."""

    def __init__(self):
        if not self.service_name:
            raise Exception("LoadConfigBuilder: missing service_name attribute.")

    def with_jobs(self, jobs: list[StrataLoadJob]):
        self.jobs.extend(jobs)
        return self

    def with_rate(self, rate: int):
        self.spawn_rate = rate
        return self

    def __call__(self, svcs) -> LoadConfig:
        if not self.jobs:
            raise Exception("LoadConfigBuilder: load jobs list is empty")

        host = self.host_url(svcs)
        # Patch jobs by the host.
        for job in self.jobs:
            job.host = host

        return LoadConfig(self.jobs, host, self.spawn_rate)

    def host_url(self, _svcs: dict[str, flexitest.Service]) -> str:
        raise NotImplementedError()

    @property
    def name(self):
        return self.service_name


class RethLoadConfigBuilder(LoadConfigBuilder):
    service_name: str = "reth"
    spawn_rate: int = 20

    def host_url(self, svcs: dict[str, flexitest.Service]) -> str:
        reth = svcs["reth"]
        web3_port = reth.get_prop("eth_rpc_http_port")
        return f"http://localhost:{web3_port}"
