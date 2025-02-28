#!/usr/bin/env python3
import os

import gevent

from load.cfg import LoadConfig
from load.reth import BasicRethBlockJob, BasicRethTxJob
from load.service import LoadGeneratorService

os.mkdir("logs")
loadgen = LoadGeneratorService(
    "logs", LoadConfig([BasicRethBlockJob, BasicRethTxJob], "http://localhost:8545", 30)
)


loadgen.start()
print("sleeping 3000 secs")
gevent.sleep(3000)
loadgen.stop()
