import grpc

import info_pb2
import info_pb2_grpc
import peering_pb2
import peering_pb2_grpc
from google.protobuf import empty_pb2
from time import time, sleep

empty = empty_pb2.Empty()

with grpc.insecure_channel('127.0.0.1:2080') as channel:
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

    print("Peering stub methods:")
    peering_stub = peering_pb2_grpc.PeeringStub(channel)

    print("Connecting to peer...")
    fake_addr = "127.0.0.1:13321"
    try:
        result = peering_stub.ConnectPeer(
            peering_pb2.ConnectPeerRequest(address=fake_addr))
    except Exception as err:
        print("Failed to connect to", fake_addr)
    real_addr = "127.0.0.1:2080"
    try:
        result = peering_stub.ConnectPeer(
            peering_pb2.ConnectPeerRequest(address=real_addr))
        print("Connected to", real_addr)
    except Exception as err:
        print("Failed to connect to", real_addr)
    print(result)
