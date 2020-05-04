import grpc

import info_pb2
import info_pb2_grpc

from google.protobuf import empty_pb2
from time import time, sleep

empty = empty_pb2.Empty()

with grpc.insecure_channel('127.0.0.1:2081') as channel:
    print("Info stub methods:")
    info_stub = info_pb2_grpc.InfoStub(channel)

    print("Pinging...")
    start = time()
    info_stub.Version(empty)
    end = time()
    print("Received in", (end - start) * 1_000, "ms")

    print("Getting version...")
    version = info_stub.Version(empty)
    print(version)

    print("Getting uptime...")
    uptime = info_stub.Uptime(empty)
    print(uptime)
