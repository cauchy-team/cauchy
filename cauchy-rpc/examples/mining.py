import grpc
import mining_pb2, mining_pb2_grpc

from google.protobuf import empty_pb2
from time import time, sleep

empty = empty_pb2.Empty()

with grpc.insecure_channel('127.0.0.1:2081') as channel:
    print("Info stub methods:")
    mining_stub = mining_pb2_grpc.MiningStub(channel)

    print("Mining Info...")
    info = mining_stub.MiningInfo(empty)
    print(info)